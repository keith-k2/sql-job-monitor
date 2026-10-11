use crate::domain::{
    format_step_progress, sql_agent_duration, timeline::TimelineHistory, HistoryEntry, Job,
    JobStatus, JobStep,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use thiserror::Error;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, Row, SqlBrowser};
use tokio::{net::TcpStream, sync::Mutex, time::timeout};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use zeroize::Zeroizing;

pub type SharedClient = Arc<Mutex<Client<Compat<TcpStream>>>>;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub enum Authentication {
    Integrated,
    SqlServer {
        username: String,
        password: Zeroizing<String>,
    },
}

#[derive(Clone, Debug)]
pub struct ConnectionOptions {
    pub server: String,
    pub port: Option<u16>,
    pub database: String,
    pub authentication: Authentication,
    pub trust_server_certificate: bool,
}

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("The server name is required.")]
    MissingServer,
    #[error("Connection timed out. Check the server name, TCP configuration, and firewall.")]
    ConnectionTimeout,
    #[error("The SQL operation timed out after 20 seconds.")]
    OperationTimeout,
    #[error("{0}")]
    Sql(String),
}

pub async fn connect(options: &ConnectionOptions) -> Result<SharedClient, MonitorError> {
    if options.server.trim().is_empty() {
        return Err(MonitorError::MissingServer);
    }

    let mut config = Config::new();
    let (host, instance) = split_instance(options.server.trim());
    config.host(host);
    config.database(if options.database.trim().is_empty() {
        "msdb"
    } else {
        options.database.trim()
    });
    config.encryption(EncryptionLevel::Required);
    if options.trust_server_certificate {
        config.trust_cert();
    }

    match &options.authentication {
        Authentication::Integrated => config.authentication(AuthMethod::Integrated),
        Authentication::SqlServer { username, password } => {
            config.authentication(AuthMethod::sql_server(username, password.as_str()))
        }
    }

    let tcp = if let Some(instance) = instance {
        config.instance_name(instance);
        config.port(1434);
        timeout(Duration::from_secs(12), TcpStream::connect_named(&config))
            .await
            .map_err(|_| MonitorError::ConnectionTimeout)?
            .map_err(classify_error)?
    } else {
        config.port(options.port.unwrap_or(1433));
        timeout(
            Duration::from_secs(12),
            TcpStream::connect(config.get_addr()),
        )
        .await
        .map_err(|_| MonitorError::ConnectionTimeout)?
        .map_err(|error| classify_message(&error.to_string()))?
    };
    tcp.set_nodelay(true)
        .map_err(|error| classify_message(&error.to_string()))?;

    let client = timeout(
        Duration::from_secs(12),
        Client::connect(config, tcp.compat_write()),
    )
    .await
    .map_err(|_| MonitorError::ConnectionTimeout)?
    .map_err(classify_error)?;

    Ok(Arc::new(Mutex::new(client)))
}

pub async fn fetch_jobs(client: &SharedClient) -> Result<Vec<Job>, MonitorError> {
    query_jobs(client, None).await
}

pub async fn fetch_job(client: &SharedClient, job_id: &str) -> Result<Option<Job>, MonitorError> {
    Ok(query_jobs(client, Some(job_id)).await?.into_iter().next())
}

async fn query_jobs(client: &SharedClient, job_id: Option<&str>) -> Result<Vec<Job>, MonitorError> {
    const JOBS_SQL: &str = r#"
WITH latest_session AS (
    SELECT MAX(session_id) AS session_id FROM msdb.dbo.sysjobactivity
)
SELECT
    CONVERT(nvarchar(36), j.job_id) AS job_id,
    j.name,
    CONVERT(int, j.enabled) AS enabled,
    CASE WHEN a.start_execution_date IS NOT NULL AND a.stop_execution_date IS NULL
         THEN 1 ELSE 0 END AS is_running,
    CONVERT(int, h.last_run_outcome) AS run_status,
    COALESCE(active_step.step_name, N'') AS current_step,
    COALESCE(active_step.step_id, 0) AS step_number,
    CONVERT(int, (
        SELECT COUNT(*) FROM msdb.dbo.sysjobsteps AS st WHERE st.job_id = j.job_id
    )) AS step_count,
    CASE WHEN h.last_run_date > 0 THEN CONVERT(nvarchar(19), DATETIMEFROMPARTS(
        h.last_run_date / 10000, h.last_run_date / 100 % 100, h.last_run_date % 100,
        h.last_run_time / 10000, h.last_run_time / 100 % 100, h.last_run_time % 100, 0), 120)
    ELSE N'' END AS last_run,
    COALESCE(h.last_run_duration, 0) AS run_duration,
    COALESCE(CONVERT(nvarchar(19), a.next_scheduled_run_date, 120), N'') AS next_run,
    COALESCE(SUSER_SNAME(j.owner_sid), N'') AS owner_name,
    COALESCE(c.name, N'') AS category_name,
    CASE WHEN a.start_execution_date IS NOT NULL AND a.stop_execution_date IS NULL
         THEN CONVERT(nvarchar(19), a.start_execution_date, 120) ELSE N'' END AS run_started
FROM msdb.dbo.sysjobs_view AS j
LEFT JOIN msdb.dbo.sysjobactivity AS a
    ON a.job_id = j.job_id
   AND a.session_id = (SELECT session_id FROM latest_session)
LEFT JOIN msdb.dbo.sysjobsteps AS prev_step
    ON prev_step.job_id = j.job_id
   AND prev_step.step_id = a.last_executed_step_id
OUTER APPLY (
    -- last_executed_step_id is the step that already finished. The running step
    -- is the job's start step until then, or the step that the finished step
    -- advances to (the next step, or an explicit "go to step").
    SELECT TOP (1) st.step_id, st.step_name
    FROM msdb.dbo.sysjobsteps AS st
    WHERE st.job_id = j.job_id
      AND st.step_id = CASE
            WHEN a.start_execution_date IS NULL
              OR a.stop_execution_date IS NOT NULL
                THEN a.last_executed_step_id
            WHEN ISNULL(a.last_executed_step_id, 0) = 0
                THEN ISNULL(j.start_step_id, 1)
            WHEN ISNULL(prev_step.on_success_action, 3) = 4
                THEN prev_step.on_success_step_id
            WHEN prev_step.on_success_action IN (1, 2)
                THEN a.last_executed_step_id
            ELSE COALESCE(
                (
                    SELECT MIN(next_step.step_id)
                    FROM msdb.dbo.sysjobsteps AS next_step
                    WHERE next_step.job_id = j.job_id
                      AND next_step.step_id > a.last_executed_step_id
                ),
                a.last_executed_step_id
            )
          END
) AS active_step
LEFT JOIN msdb.dbo.syscategories AS c
    ON c.category_id = j.category_id
OUTER APPLY (
    SELECT TOP (1) js.last_run_outcome, js.last_run_date, js.last_run_time, js.last_run_duration
    FROM msdb.dbo.sysjobservers AS js
    WHERE js.job_id = j.job_id
    ORDER BY js.last_run_date DESC, js.last_run_time DESC
) AS h
WHERE @P1 IS NULL OR j.job_id = CONVERT(uniqueidentifier, @P1)
ORDER BY j.name;"#;

    let mut guard = client.lock().await;
    let rows = timeout(OPERATION_TIMEOUT, guard.query(JOBS_SQL, &[&job_id]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?
        .into_first_result()
        .await
        .map_err(classify_error)?;

    let mut jobs = rows
        .iter()
        .map(job_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    drop(guard);
    overlay_agent_steps(client, &mut jobs).await;
    Ok(jobs)
}

/// Ask SQL Agent which step is executing. This matches start-at-step, retries,
/// and jumps. A missing permission leaves the inferred step from the job query.
async fn overlay_agent_steps(client: &SharedClient, jobs: &mut [Job]) {
    if !jobs.iter().any(|job| job.running) {
        return;
    }
    let Ok(Ok(steps)) = timeout(Duration::from_secs(5), agent_running_steps(client)).await else {
        return;
    };
    for job in jobs.iter_mut().filter(|job| job.running) {
        let Some((step_number, step_name)) = steps.get(&job.id) else {
            continue;
        };
        if *step_number <= 0 || step_name.is_empty() {
            continue;
        }
        job.step_number = *step_number;
        job.step_name.clone_from(step_name);
        job.current_step = format_step_progress(step_name, *step_number, job.step_count);
    }
}

async fn agent_running_steps(
    client: &SharedClient,
) -> Result<HashMap<String, (i32, String)>, MonitorError> {
    const SQL: &str = r#"
IF OBJECT_ID(N'tempdb..#sql_job_monitor_steps') IS NOT NULL
    DROP TABLE #sql_job_monitor_steps;
CREATE TABLE #sql_job_monitor_steps (
    job_id uniqueidentifier NOT NULL,
    last_run_date int NOT NULL,
    last_run_time int NOT NULL,
    next_run_date int NOT NULL,
    next_run_time int NOT NULL,
    next_run_schedule_id int NOT NULL,
    requested_to_run int NOT NULL,
    request_source int NOT NULL,
    request_source_id sysname COLLATE database_default NULL,
    running int NOT NULL,
    current_step int NOT NULL,
    current_retry_attempt int NOT NULL,
    job_state int NOT NULL
);
BEGIN TRY
    DECLARE @is_sysadmin int = ISNULL(IS_SRVROLEMEMBER(N'sysadmin'), 0);
    DECLARE @owner sysname = SUSER_SNAME();
    INSERT INTO #sql_job_monitor_steps
    EXEC master.dbo.xp_sqlagent_enum_jobs @is_sysadmin, @owner;
END TRY
BEGIN CATCH
END CATCH;
SELECT
    CONVERT(nvarchar(36), agent.job_id) AS job_id,
    CONVERT(int, agent.current_step) AS step_number,
    COALESCE(step.step_name, N'') AS step_name
FROM #sql_job_monitor_steps AS agent
LEFT JOIN msdb.dbo.sysjobsteps AS step
    ON step.job_id = agent.job_id
   AND step.step_id = agent.current_step
WHERE agent.running = 1
  AND agent.current_step > 0;
DROP TABLE #sql_job_monitor_steps;"#;

    let mut guard = client.lock().await;
    let params: &[&dyn tiberius::ToSql] = &[];
    let sets = timeout(Duration::from_secs(5), guard.query(SQL, params))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?
        .into_results()
        .await
        .map_err(classify_error)?;
    let mut steps = HashMap::new();
    for rows in &sets {
        if rows
            .first()
            .is_none_or(|row| row.try_get::<&str, _>("job_id").is_err())
        {
            continue;
        }
        for row in rows {
            let id = text(row, "job_id")?;
            let step_number = integer(row, "step_number")?;
            let step_name = text(row, "step_name")?;
            if step_number > 0 && !step_name.is_empty() {
                steps.insert(id, (step_number, step_name));
            }
        }
    }
    Ok(steps)
}

pub async fn fetch_history(
    client: &SharedClient,
    job_id: &str,
) -> Result<Vec<HistoryEntry>, MonitorError> {
    const SQL: &str = r#"
SELECT TOP (1000)
    step_id,
    CASE WHEN run_date > 0 THEN CONVERT(nvarchar(19), DATETIMEFROMPARTS(
        run_date / 10000, run_date / 100 % 100, run_date % 100,
        run_time / 10000, run_time / 100 % 100, run_time % 100, 0), 120)
    ELSE N'' END AS run_at,
    CASE WHEN step_id = 0 THEN N'(Job outcome)'
         ELSE N'Step ' + CONVERT(nvarchar(10), step_id) + N': ' + step_name END AS step_name,
    CASE run_status WHEN 0 THEN N'Failed' WHEN 1 THEN N'Succeeded'
         WHEN 2 THEN N'Retry' WHEN 3 THEN N'Canceled'
         WHEN 4 THEN N'In progress' ELSE N'Unknown' END AS outcome,
    run_duration,
    COALESCE(message, N'') AS message
FROM msdb.dbo.sysjobhistory
WHERE job_id = CONVERT(uniqueidentifier, @P1)
ORDER BY instance_id DESC;"#;

    let mut guard = client.lock().await;
    let rows = timeout(OPERATION_TIMEOUT, guard.query(SQL, &[&job_id]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?
        .into_first_result()
        .await
        .map_err(classify_error)?;

    rows.iter().map(history_from_row).collect()
}

/// History for every job since `since_date` (yyyymmdd), newest-first per job.
/// Reads only the newest slice of sysjobhistory by its clustered instance_id,
/// so large history tables are never scanned end to end.
pub async fn fetch_timeline(
    client: &SharedClient,
    since_date: i32,
) -> Result<TimelineHistory, MonitorError> {
    const SQL: &str = r#"
DECLARE @newest int = (SELECT MAX(instance_id) FROM msdb.dbo.sysjobhistory);
SELECT
    CONVERT(nvarchar(36), job_id) AS job_id,
    step_id,
    CONVERT(nvarchar(19), DATETIMEFROMPARTS(
        run_date / 10000, run_date / 100 % 100, run_date % 100,
        run_time / 10000, run_time / 100 % 100, run_time % 100, 0), 120) AS run_at,
    CASE WHEN step_id = 0 THEN N'(Job outcome)'
         ELSE N'Step ' + CONVERT(nvarchar(10), step_id) + N': ' + step_name END AS step_name,
    CASE run_status WHEN 0 THEN N'Failed' WHEN 1 THEN N'Succeeded'
         WHEN 2 THEN N'Retry' WHEN 3 THEN N'Canceled'
         WHEN 4 THEN N'In progress' ELSE N'Unknown' END AS outcome,
    run_duration
FROM msdb.dbo.sysjobhistory
WHERE instance_id > ISNULL(@newest, 0) - 150000
  AND run_date >= @P1
ORDER BY instance_id DESC;"#;

    let mut guard = client.lock().await;
    let rows = timeout(OPERATION_TIMEOUT, guard.query(SQL, &[&since_date]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?
        .into_first_result()
        .await
        .map_err(classify_error)?;

    let mut history = TimelineHistory::new();
    for row in &rows {
        let entry = HistoryEntry {
            step_id: integer(row, "step_id")?,
            run_at: text(row, "run_at")?,
            step: text(row, "step_name")?,
            outcome: text(row, "outcome")?,
            duration_seconds: sql_agent_duration(integer(row, "run_duration")?),
            message: String::new(),
        };
        history.entry(text(row, "job_id")?).or_default().push(entry);
    }
    Ok(history)
}

pub async fn fetch_steps(
    client: &SharedClient,
    job_id: &str,
) -> Result<Vec<JobStep>, MonitorError> {
    const SQL: &str = r#"
SELECT step_id, step_name
FROM msdb.dbo.sysjobsteps
WHERE job_id = CONVERT(uniqueidentifier, @P1)
ORDER BY step_id;"#;

    let mut guard = client.lock().await;
    let rows = timeout(OPERATION_TIMEOUT, guard.query(SQL, &[&job_id]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?
        .into_first_result()
        .await
        .map_err(classify_error)?;

    rows.iter()
        .map(|row| {
            Ok(JobStep {
                step_id: integer(row, "step_id")?,
                name: text(row, "step_name")?,
            })
        })
        .collect()
}

pub async fn start_job(client: &SharedClient, job_id: &str) -> Result<(), MonitorError> {
    run_command(
        client,
        "DECLARE @id uniqueidentifier = CONVERT(uniqueidentifier, @P1); EXEC msdb.dbo.sp_start_job @job_id = @id;",
        job_id,
    )
    .await
}

pub async fn start_job_at_step(
    client: &SharedClient,
    job_id: &str,
    step_name: &str,
) -> Result<(), MonitorError> {
    const SQL: &str = r#"
DECLARE @id uniqueidentifier = CONVERT(uniqueidentifier, @P1);
EXEC msdb.dbo.sp_start_job @job_id = @id, @step_name = @P2;"#;
    let mut guard = client.lock().await;
    timeout(
        OPERATION_TIMEOUT,
        guard.execute(SQL, &[&job_id, &step_name]),
    )
    .await
    .map_err(|_| MonitorError::OperationTimeout)?
    .map_err(classify_error)?;
    Ok(())
}

pub async fn stop_job(client: &SharedClient, job_id: &str) -> Result<(), MonitorError> {
    run_command(
        client,
        "DECLARE @id uniqueidentifier = CONVERT(uniqueidentifier, @P1); EXEC msdb.dbo.sp_stop_job @job_id = @id;",
        job_id,
    )
    .await
}

pub async fn set_job_enabled(
    client: &SharedClient,
    job_id: &str,
    enabled: bool,
) -> Result<(), MonitorError> {
    const SQL: &str = r#"
DECLARE @id uniqueidentifier = CONVERT(uniqueidentifier, @P1);
DECLARE @enabled tinyint = CONVERT(tinyint, @P2);
EXEC msdb.dbo.sp_update_job @job_id = @id, @enabled = @enabled;"#;
    let enabled: i32 = if enabled { 1 } else { 0 };
    let mut guard = client.lock().await;
    timeout(OPERATION_TIMEOUT, guard.execute(SQL, &[&job_id, &enabled]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?;
    Ok(())
}

async fn run_command(client: &SharedClient, sql: &str, job_id: &str) -> Result<(), MonitorError> {
    let mut guard = client.lock().await;
    timeout(OPERATION_TIMEOUT, guard.execute(sql, &[&job_id]))
        .await
        .map_err(|_| MonitorError::OperationTimeout)?
        .map_err(classify_error)?;
    Ok(())
}

fn job_from_row(row: &Row) -> Result<Job, MonitorError> {
    let enabled = integer(row, "enabled")? != 0;
    let running = integer(row, "is_running")? != 0;
    let run_status = row
        .try_get::<i32, _>("run_status")
        .map_err(classify_error)?;
    let step_name = text(row, "current_step")?;
    let step_number = integer(row, "step_number")?;
    let step_count = integer(row, "step_count")?;
    Ok(Job {
        id: text(row, "job_id")?,
        name: text(row, "name")?,
        enabled,
        running,
        status: JobStatus::from_database(enabled, running, run_status),
        step_name: step_name.clone(),
        step_number,
        step_count,
        run_started: text(row, "run_started")?,
        current_step: format_step_progress(&step_name, step_number, step_count),
        last_run: text(row, "last_run")?,
        last_duration_seconds: sql_agent_duration(integer(row, "run_duration")?),
        next_run: text(row, "next_run")?,
        owner: text(row, "owner_name")?,
        category: text(row, "category_name")?,
    })
}

fn history_from_row(row: &Row) -> Result<HistoryEntry, MonitorError> {
    Ok(HistoryEntry {
        step_id: integer(row, "step_id")?,
        run_at: text(row, "run_at")?,
        step: text(row, "step_name")?,
        outcome: text(row, "outcome")?,
        duration_seconds: sql_agent_duration(integer(row, "run_duration")?),
        message: text(row, "message")?,
    })
}

fn text(row: &Row, column: &str) -> Result<String, MonitorError> {
    row.try_get::<&str, _>(column)
        .map(|value| value.unwrap_or_default().to_owned())
        .map_err(classify_error)
}

fn integer(row: &Row, column: &str) -> Result<i32, MonitorError> {
    row.try_get::<i32, _>(column)
        .map(|value| value.unwrap_or_default())
        .map_err(classify_error)
}

fn split_instance(server: &str) -> (&str, Option<&str>) {
    match server.split_once('\\') {
        Some((host, instance)) if !host.is_empty() && !instance.is_empty() => {
            (host, Some(instance))
        }
        _ => (server, None),
    }
}

fn classify_error(error: tiberius::error::Error) -> MonitorError {
    classify_message(&error.to_string())
}

fn classify_message(message: &str) -> MonitorError {
    let lower = message.to_lowercase();
    let friendly = if lower.contains("login failed") {
        "Login failed. Verify the authentication method and credentials."
    } else if lower.contains("certificate") || lower.contains("tls") {
        "TLS validation failed. Install a trusted server certificate, or explicitly enable the certificate override for this connection."
    } else if lower.contains("sqlserveragent")
        || lower.contains("sql server agent")
        || lower.contains("agent xp")
    {
        "SQL Server Agent is unavailable or disabled on this server."
    } else if lower.contains("permission") || lower.contains("denied") {
        "Permission denied. Monitoring requires an msdb SQLAgentReaderRole-compatible account; controls require SQLAgentOperatorRole or equivalent rights."
    } else if lower.contains("browser") {
        "The named instance could not be resolved through SQL Server Browser. Verify that the service and UDP 1434 are available."
    } else {
        return MonitorError::Sql(sanitize(message));
    };
    MonitorError::Sql(format!("{friendly} Details: {}", sanitize(message)))
}

fn sanitize(message: &str) -> String {
    let compact = message.replace(['\r', '\n'], " ");
    compact.chars().take(600).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_named_instance() {
        assert_eq!(
            split_instance(r"SERVER\REPORTING"),
            ("SERVER", Some("REPORTING"))
        );
        assert_eq!(split_instance("SERVER"), ("SERVER", None));
    }

    #[test]
    fn sanitizes_multiline_errors() {
        assert_eq!(sanitize("first\r\nsecond"), "first  second");
    }
}
