use sql_job_monitor::domain::{
    category_summary, command_availability, filter_and_sort, format_duration, format_step_progress,
    format_timestamp, group_runs, include_active_run, last_ran, local_epoch, next_run_in,
    relative_update, sql_agent_duration, status_counts, ActiveRun, HistoryEntry, HistoryRun, Job,
    JobStatus, SortColumn,
};

fn entry(step_id: i32, run_at: &str, outcome: &str, duration_seconds: u32) -> HistoryEntry {
    HistoryEntry {
        step_id,
        run_at: run_at.to_owned(),
        step: format!("step {step_id}"),
        outcome: outcome.to_owned(),
        duration_seconds,
        message: String::new(),
    }
}

#[test]
fn formats_timestamps_with_twelve_hour_clock() {
    assert_eq!(
        format_timestamp("2026-10-09 13:05:07"),
        "2026-10-09 1:05:07 PM"
    );
    assert_eq!(
        format_timestamp("2026-10-09 00:00:00"),
        "2026-10-09 12:00:00 AM"
    );
    assert_eq!(format_timestamp(""), "");
}

#[test]
fn groups_history_rows_into_runs() {
    let rows = vec![
        entry(1, "2026-10-09 13:00:00", "In progress", 5),
        entry(0, "2026-10-09 12:00:00", "Succeeded", 30),
        entry(2, "2026-10-09 12:00:20", "Succeeded", 10),
        entry(1, "2026-10-09 12:00:00", "Succeeded", 20),
        entry(0, "2026-10-08 12:00:00", "Failed", 4),
        entry(1, "2026-10-08 12:00:00", "Failed", 4),
    ];

    let runs = group_runs(rows, 50);
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].outcome, "In progress");
    assert_eq!(runs[0].run_at, "2026-10-09 13:00:00");
    assert_eq!(runs[1].outcome, "Succeeded");
    assert_eq!(
        runs[1]
            .steps
            .iter()
            .map(|step| step.step_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(runs[2].outcome, "Failed");
    assert_eq!(group_runs(vec![entry(0, "", "Failed", 1)], 0).len(), 0);
}

fn job(name: &str, status: JobStatus, owner: &str, category: &str) -> Job {
    Job {
        id: name.to_owned(),
        name: name.to_owned(),
        enabled: status != JobStatus::Disabled,
        status,
        owner: owner.to_owned(),
        category: category.to_owned(),
        ..Job::default()
    }
}

#[test]
fn describes_last_and_next_run_times() {
    assert_eq!(last_ran(180), "Last ran 3 min ago");
    assert_eq!(last_ran(8), "Last ran 8 seconds ago");
    assert_eq!(next_run_in(300), "Next run in 5 min");
    assert_eq!(next_run_in(-180), "Next run was due 3 min ago");
    assert_eq!(local_epoch(""), 0);
}

#[test]
fn describes_how_long_ago_the_list_was_updated() {
    assert_eq!(relative_update(2), "Updated just now");
    assert_eq!(relative_update(8), "Updated 8 seconds ago");
    assert_eq!(relative_update(60), "Updated 1 min ago");
    assert_eq!(relative_update(180), "Updated 3 min ago");
    assert_eq!(relative_update(7200), "Updated 2 hours ago");
}

#[test]
fn maps_agent_status_with_runtime_precedence() {
    assert_eq!(
        JobStatus::from_database(true, true, Some(1)),
        JobStatus::Running
    );
    assert_eq!(
        JobStatus::from_database(false, false, Some(1)),
        JobStatus::Disabled
    );
    assert_eq!(
        JobStatus::from_database(true, false, Some(0)),
        JobStatus::Failed
    );
    assert_eq!(
        JobStatus::from_database(true, false, Some(3)),
        JobStatus::Canceled
    );
}

#[test]
fn converts_sql_agent_hhmmss_duration() {
    assert_eq!(sql_agent_duration(10203), 3_723);
    assert_eq!(format_duration(3_723), "01:02:03");
    assert_eq!(sql_agent_duration(-1), 0);
}

#[test]
fn filters_searches_and_sorts_jobs() {
    let jobs = vec![
        job("Warehouse Load", JobStatus::Succeeded, "etl", "ETL"),
        job("Backup", JobStatus::Failed, "dba", "Database Maintenance"),
        job("Cleanup", JobStatus::Succeeded, "dba", "ETL"),
    ];
    let categories = ["ETL".to_owned(), "Database Maintenance".to_owned()];

    let filtered = filter_and_sort(
        &jobs,
        "dba",
        "Succeeded",
        &categories,
        SortColumn::Name,
        true,
    );
    assert_eq!(
        filtered
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Cleanup"]
    );

    let sorted = filter_and_sort(&jobs, "", "All", &categories, SortColumn::Name, false);
    assert_eq!(
        sorted
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Warehouse Load", "Cleanup", "Backup"]
    );

    let etl_only = filter_and_sort(
        &jobs,
        "",
        "All",
        &["ETL".to_owned()],
        SortColumn::Name,
        true,
    );
    assert_eq!(
        etl_only
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Cleanup", "Warehouse Load"]
    );
}

#[test]
fn shows_the_running_step_in_history() {
    let previous = HistoryRun {
        run_at: "2026-10-09 12:00:00".to_owned(),
        outcome: "Succeeded".to_owned(),
        duration_seconds: 30,
        message: String::new(),
        steps: vec![entry(1, "2026-10-09 12:00:00", "Succeeded", 30)],
    };
    let active = ActiveRun {
        started_at: String::new(),
        step_number: 2,
        step_count: 8,
        step_name: "Load facts".to_owned(),
    };

    let runs = include_active_run(vec![previous], Some(active));
    assert_eq!(runs[0].outcome, "Running");
    assert_eq!(runs[0].steps[0].step, "Step 2 of 8: Load facts");
    assert_eq!(runs[0].steps[0].outcome, "Running");
    assert_eq!(runs[1].outcome, "Succeeded");

    let open = group_runs(
        vec![
            entry(1, "2026-10-09 13:00:00", "Succeeded", 5),
            entry(0, "2026-10-09 12:00:00", "Succeeded", 30),
        ],
        50,
    );
    let active = ActiveRun {
        started_at: "2026-10-09 13:00:00".to_owned(),
        step_number: 2,
        step_count: 8,
        step_name: "Load facts".to_owned(),
    };
    let runs = include_active_run(open, Some(active));
    assert_eq!(runs[0].outcome, "Running");
    assert_eq!(
        runs[0]
            .steps
            .iter()
            .map(|step| step.step_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(runs[0].steps[1].outcome, "Running");

    let named = include_active_run(
        Vec::new(),
        Some(ActiveRun {
            started_at: String::new(),
            step_number: 0,
            step_count: 4,
            step_name: "Load facts".to_owned(),
        }),
    );
    assert_eq!(named[0].steps[0].step, "Load facts");
    assert_ne!(named[0].steps[0].step, "Starting");
    assert!(include_active_run(runs, None)
        .iter()
        .any(|run| run.outcome == "Running"));
}

#[test]
fn builds_timeline_bars_for_runs_and_steps() {
    use chrono::NaiveDateTime;
    use sql_job_monitor::domain::timeline::{build_timeline, TimelineHistory};

    let at = |raw: &str| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").unwrap();
    let job = Job {
        id: "A".to_owned(),
        name: "Load".to_owned(),
        status: JobStatus::Succeeded,
        enabled: true,
        ..Job::default()
    };
    let mut history = TimelineHistory::new();
    history.insert(
        "A".to_owned(),
        vec![
            entry(0, "2026-10-09 10:00:00", "Succeeded", 600),
            entry(2, "2026-10-09 10:04:00", "Failed", 360),
            entry(1, "2026-10-09 10:00:00", "Succeeded", 240),
            entry(0, "2026-10-08 01:00:00", "Succeeded", 60),
        ],
    );
    let lanes = build_timeline(
        &[job],
        &history,
        at("2026-10-09 09:00:00"),
        at("2026-10-09 12:00:00"),
        at("2026-10-09 11:00:00"),
    );
    let bars = &lanes[0].bars;
    assert_eq!(bars.len(), 3, "the run outside the window is dropped");
    assert!(!bars[0].is_step);
    assert_eq!((bars[0].start, bars[0].length), (3600.0, 600.0));
    assert_eq!(bars[1].status, "Succeeded");
    assert_eq!((bars[2].start, bars[2].length), (3840.0, 360.0));
    assert_eq!(bars[2].status, "Failed");
}

#[test]
fn counts_each_status_and_skips_empty_ones() {
    let jobs = vec![
        Job {
            status: JobStatus::Running,
            ..Job::default()
        },
        Job {
            status: JobStatus::Running,
            ..Job::default()
        },
        Job {
            status: JobStatus::Failed,
            ..Job::default()
        },
        Job {
            status: JobStatus::Succeeded,
            ..Job::default()
        },
    ];
    assert_eq!(
        status_counts(&jobs),
        vec![
            ("Running".to_owned(), 2),
            ("Succeeded".to_owned(), 1),
            ("Failed".to_owned(), 1),
        ]
    );
}

#[test]
fn keeps_a_failed_count_when_nothing_has_failed() {
    let jobs = vec![Job {
        status: JobStatus::Succeeded,
        ..Job::default()
    }];
    assert_eq!(
        status_counts(&jobs),
        vec![("Succeeded".to_owned(), 1), ("Failed".to_owned(), 0)]
    );
}

#[test]
fn pins_running_jobs_above_the_selected_sort() {
    let jobs = vec![
        job("Alpha", JobStatus::Succeeded, "dba", "ETL"),
        job("Middle", JobStatus::Running, "dba", "ETL"),
        job("Zebra", JobStatus::Failed, "dba", "ETL"),
    ];
    let categories = ["ETL".to_owned()];
    let ascending = filter_and_sort(&jobs, "", "All", &categories, SortColumn::Name, true);
    assert_eq!(
        ascending
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Middle", "Alpha", "Zebra"]
    );
    let descending = filter_and_sort(&jobs, "", "All", &categories, SortColumn::Name, false);
    assert_eq!(
        descending
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Middle", "Zebra", "Alpha"]
    );
}

#[test]
fn greys_out_job_commands_that_do_not_apply() {
    let idle = command_availability(true, false, false);
    assert!(idle.start && idle.disable && idle.refresh);
    assert!(!idle.stop && !idle.enable);

    let running = command_availability(true, true, false);
    assert!(running.stop && running.disable && running.refresh);
    assert!(!running.start && !running.enable);

    let disabled = command_availability(false, false, false);
    assert!(disabled.enable && disabled.refresh);
    assert!(!disabled.start && !disabled.stop && !disabled.disable);

    let busy = command_availability(true, true, true);
    assert!(!busy.start && !busy.stop && !busy.enable && !busy.disable && !busy.refresh);
}

#[test]
fn formats_step_progress_and_category_summary() {
    assert_eq!(
        format_step_progress("Load facts", 2, 8),
        "Step 2 of 8: Load facts"
    );
    assert_eq!(format_step_progress("", 2, 8), "Step 2 of 8");
    assert_eq!(format_step_progress("Load facts", 0, 8), "Load facts");
    assert_eq!(category_summary(&["ETL".to_owned()]), "ETL");
    assert_eq!(category_summary(&[]), "No categories");
}
