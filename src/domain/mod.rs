use std::cmp::Ordering;

pub mod timeline;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub status: JobStatus,
    pub current_step: String,
    pub last_run: String,
    pub last_duration_seconds: u32,
    pub next_run: String,
    pub owner: String,
    pub category: String,
    pub running: bool,
    pub step_number: i32,
    pub step_count: i32,
    pub step_name: String,
    pub run_started: String,
}

impl Job {
    pub fn active_run(&self) -> Option<ActiveRun> {
        if !self.running {
            return None;
        }
        Some(ActiveRun {
            started_at: self.run_started.clone(),
            step_number: self.step_number,
            step_count: self.step_count,
            step_name: self.step_name.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobStep {
    pub step_id: i32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveRun {
    pub started_at: String,
    pub step_number: i32,
    pub step_count: i32,
    pub step_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandAvailability {
    pub start: bool,
    pub stop: bool,
    pub enable: bool,
    pub disable: bool,
    pub refresh: bool,
}

pub fn command_availability(enabled: bool, running: bool, busy: bool) -> CommandAvailability {
    CommandAvailability {
        start: enabled && !running && !busy,
        stop: running && !busy,
        enable: !enabled && !busy,
        disable: enabled && !busy,
        refresh: !busy,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Succeeded,
    Failed,
    Retry,
    Canceled,
    Disabled,
    #[default]
    Unknown,
}

impl JobStatus {
    pub fn from_database(enabled: bool, running: bool, run_status: Option<i32>) -> Self {
        if !enabled {
            return Self::Disabled;
        }
        if running {
            return Self::Running;
        }
        match run_status {
            Some(0) => Self::Failed,
            Some(1) => Self::Succeeded,
            Some(2) => Self::Retry,
            Some(3) => Self::Canceled,
            Some(4) => Self::Running,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Retry => "Retry",
            Self::Canceled => "Canceled",
            Self::Disabled => "Disabled",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryEntry {
    pub step_id: i32,
    pub run_at: String,
    pub step: String,
    pub outcome: String,
    pub duration_seconds: u32,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryRun {
    pub run_at: String,
    pub outcome: String,
    pub duration_seconds: u32,
    pub message: String,
    pub steps: Vec<HistoryEntry>,
}

/// Groups history rows ordered newest-first into runs. SQL Server Agent writes a
/// step 0 outcome row after a run's step rows, so a run's steps follow its outcome
/// row in descending order; steps before the first outcome row are still running.
pub fn group_runs(entries: Vec<HistoryEntry>, max_runs: usize) -> Vec<HistoryRun> {
    let mut runs: Vec<HistoryRun> = Vec::new();
    let mut current: Option<HistoryRun> = None;

    for entry in entries {
        if entry.step_id == 0 {
            if let Some(run) = current.take() {
                runs.push(run);
                if runs.len() == max_runs {
                    return finish_runs(runs);
                }
            }
            current = Some(HistoryRun {
                run_at: entry.run_at,
                outcome: entry.outcome,
                duration_seconds: entry.duration_seconds,
                message: entry.message,
                steps: Vec::new(),
            });
        } else {
            current
                .get_or_insert_with(|| HistoryRun {
                    outcome: "In progress".to_owned(),
                    ..HistoryRun::default()
                })
                .steps
                .push(entry);
        }
    }
    if let Some(run) = current {
        if runs.len() < max_runs {
            runs.push(run);
        }
    }
    finish_runs(runs)
}

/// When a job is executing, history often has no row yet for the step that is
/// running. Keep that execution at the top of the history and name its step.
pub fn include_active_run(mut runs: Vec<HistoryRun>, active: Option<ActiveRun>) -> Vec<HistoryRun> {
    let Some(active) = active else {
        return runs;
    };
    let step_label = active_step_label(&active);
    let open = runs.first().is_some_and(|run| run.outcome == "In progress");
    if open {
        let run = &mut runs[0];
        run.outcome = "Running".to_owned();
        if run.run_at.is_empty() {
            run.run_at.clone_from(&active.started_at);
        }
        let elapsed = elapsed_seconds(&active.started_at);
        if elapsed > 0 {
            run.duration_seconds = elapsed;
        }
        attach_active_step(run, &active, &step_label);
    } else {
        runs.insert(
            0,
            HistoryRun {
                run_at: active.started_at.clone(),
                outcome: "Running".to_owned(),
                duration_seconds: elapsed_seconds(&active.started_at),
                message: String::new(),
                steps: vec![active_step(&active, &step_label)],
            },
        );
    }
    runs
}

fn active_step_label(active: &ActiveRun) -> String {
    if active.step_number <= 0 {
        if active.step_name.is_empty() {
            return "Starting".to_owned();
        }
        return active.step_name.clone();
    }
    let label = format_step_progress(&active.step_name, active.step_number, active.step_count);
    if label.is_empty() {
        format!("Step {}", active.step_number)
    } else {
        label
    }
}

fn attach_active_step(run: &mut HistoryRun, active: &ActiveRun, step_label: &str) {
    if active.step_number <= 0 {
        if run.steps.is_empty() {
            run.steps.push(active_step(active, step_label));
        }
        return;
    }
    if let Some(step) = run
        .steps
        .iter_mut()
        .find(|step| step.step_id == active.step_number)
    {
        if step.outcome == "In progress" {
            step.outcome = "Running".to_owned();
            step.step = step_label.to_owned();
        }
        return;
    }
    run.steps.push(active_step(active, step_label));
    run.steps.sort_by_key(|step| step.step_id);
}

fn active_step(active: &ActiveRun, step_label: &str) -> HistoryEntry {
    HistoryEntry {
        step_id: active.step_number,
        run_at: active.started_at.clone(),
        step: step_label.to_owned(),
        outcome: "Running".to_owned(),
        duration_seconds: 0,
        message: String::new(),
    }
}

fn elapsed_seconds(started_at: &str) -> u32 {
    let Ok(started) = chrono::NaiveDateTime::parse_from_str(started_at.trim(), "%Y-%m-%d %H:%M:%S")
    else {
        return 0;
    };
    let elapsed = chrono::Local::now().naive_local() - started;
    u32::try_from(elapsed.num_seconds()).unwrap_or(0)
}

fn finish_runs(mut runs: Vec<HistoryRun>) -> Vec<HistoryRun> {
    for run in &mut runs {
        run.steps.sort_by_key(|step| step.step_id);
        if run.run_at.is_empty() {
            if let Some(first) = run.steps.first() {
                run.run_at = first.run_at.clone();
            }
            run.duration_seconds = run.steps.iter().map(|step| step.duration_seconds).sum();
        }
    }
    runs
}

/// Converts `yyyy-MM-dd HH:mm:ss` from SQL Server into `yyyy-MM-dd h:mm:ss AM/PM`.
pub fn format_timestamp(raw: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%d %H:%M:%S")
        .map(|value| value.format("%Y-%m-%d %-I:%M:%S %p").to_string())
        .unwrap_or_else(|_| raw.to_owned())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    #[default]
    Name,
    Status,
    LastRun,
}

/// Counts jobs by status. Empty statuses are omitted, except Failed, which
/// stays visible at zero so a failure is never hidden by another filter.
/// Order matches the status chips: running and outcomes first, then disabled.
pub fn status_counts(jobs: &[Job]) -> Vec<(String, i32)> {
    const ORDER: [JobStatus; 7] = [
        JobStatus::Running,
        JobStatus::Succeeded,
        JobStatus::Failed,
        JobStatus::Retry,
        JobStatus::Canceled,
        JobStatus::Disabled,
        JobStatus::Unknown,
    ];
    ORDER
        .into_iter()
        .filter_map(|status| {
            let count = jobs.iter().filter(|job| job.status == status).count();
            let count = i32::try_from(count).unwrap_or(i32::MAX);
            (count > 0 || status == JobStatus::Failed).then(|| (status.label().to_owned(), count))
        })
        .collect()
}

pub fn filter_and_sort(
    jobs: &[Job],
    search: &str,
    status: &str,
    categories: &[String],
    column: SortColumn,
    ascending: bool,
) -> Vec<Job> {
    let search = search.trim().to_lowercase();
    let mut visible: Vec<Job> = jobs
        .iter()
        .filter(|job| {
            (search.is_empty()
                || job.name.to_lowercase().contains(&search)
                || job.owner.to_lowercase().contains(&search)
                || job.category.to_lowercase().contains(&search))
                && (status == "All" || job.status.label() == status)
                && categories
                    .iter()
                    .any(|category| category.eq_ignore_ascii_case(&job.category))
        })
        .cloned()
        .collect();

    visible.sort_by(|a, b| {
        match (
            a.status == JobStatus::Running,
            b.status == JobStatus::Running,
        ) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                let ordering = match column {
                    SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortColumn::Status => a.status.label().cmp(b.status.label()),
                    SortColumn::LastRun => a.last_run.cmp(&b.last_run),
                };
                if ascending {
                    ordering
                } else {
                    reverse(ordering)
                }
            }
        }
    });
    visible
}

fn reverse(ordering: Ordering) -> Ordering {
    match ordering {
        Ordering::Less => Ordering::Greater,
        Ordering::Greater => Ordering::Less,
        Ordering::Equal => Ordering::Equal,
    }
}

pub fn format_step_progress(step_name: &str, step_number: i32, step_count: i32) -> String {
    let progress = if step_number > 0 && step_count > 0 {
        format!("Step {step_number} of {step_count}")
    } else {
        String::new()
    };
    match (progress.is_empty(), step_name.is_empty()) {
        (true, true) => String::new(),
        (true, false) => step_name.to_owned(),
        (false, true) => progress,
        (false, false) => format!("{progress}: {step_name}"),
    }
}

pub fn category_summary(selected: &[String]) -> String {
    match selected {
        [] => "No categories".to_owned(),
        [category] => category.clone(),
        categories if categories.len() <= 3 => categories.join(", "),
        categories => format!("{} categories", categories.len()),
    }
}

pub fn format_duration(total_seconds: u32) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

pub fn sql_agent_duration(raw: i32) -> u32 {
    let raw = raw.max(0) as u32;
    let hours = raw / 10_000;
    let minutes = (raw / 100) % 100;
    let seconds = raw % 100;
    hours * 3600 + minutes * 60 + seconds
}
