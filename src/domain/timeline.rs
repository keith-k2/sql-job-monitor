use super::{format_duration, group_runs, include_active_run, HistoryEntry, HistoryRun, Job};
use chrono::NaiveDateTime;
use std::collections::HashMap;

const SQL_TIME: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineBar {
    /// Seconds from the start of the window.
    pub start: f32,
    pub length: f32,
    pub status: String,
    pub label: String,
    pub detail: String,
    pub is_step: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineLane {
    pub job_id: String,
    pub name: String,
    pub status: String,
    pub current_step: String,
    pub bars: Vec<TimelineBar>,
}

/// History rows for many jobs, keyed by job id and ordered newest-first.
pub type TimelineHistory = HashMap<String, Vec<HistoryEntry>>;

/// One lane per job with a bar per run and a bar per step, clipped to the window.
pub fn build_timeline(
    jobs: &[Job],
    history: &TimelineHistory,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    now: NaiveDateTime,
) -> Vec<TimelineLane> {
    jobs.iter()
        .map(|job| {
            let entries = history.get(&job.id).cloned().unwrap_or_default();
            let runs = include_active_run(group_runs(entries, usize::MAX), job.active_run());
            let mut bars = Vec::new();
            for run in &runs {
                push_run(&mut bars, run, window_start, window_end, now);
            }
            TimelineLane {
                job_id: job.id.clone(),
                name: job.name.clone(),
                status: job.status.label().to_owned(),
                current_step: job.current_step.clone(),
                bars,
            }
        })
        .collect()
}

fn push_run(
    bars: &mut Vec<TimelineBar>,
    run: &HistoryRun,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    now: NaiveDateTime,
) {
    let Some(run_start) = parse(&run.run_at) else {
        return;
    };
    let running = run.outcome == "Running" || run.outcome == "In progress";
    let run_seconds = if running {
        seconds_between(run_start, now)
    } else {
        i64::from(run.duration_seconds)
    };
    let run_status = if running {
        "Running"
    } else {
        run.outcome.as_str()
    };

    let mut step_end = run_start;
    let mut step_bars = Vec::new();
    for step in &run.steps {
        let step_running = step.outcome == "Running" || step.outcome == "In progress";
        // A running step has no history row yet, so it starts where the
        // last finished step ended.
        let start = if step_running {
            step_end
        } else {
            parse(&step.run_at).unwrap_or(step_end)
        };
        let seconds = if step_running {
            seconds_between(start, now)
        } else {
            i64::from(step.duration_seconds)
        };
        let status = if step_running {
            "Running"
        } else {
            step.outcome.as_str()
        };
        step_end = step_end.max(start + chrono::Duration::seconds(seconds));
        if let Some(bar) = clip(
            start,
            seconds,
            window_start,
            window_end,
            status,
            &step.step,
            true,
        ) {
            step_bars.push(bar);
        }
    }

    let run_seconds = run_seconds.max(seconds_between(run_start, step_end));
    if let Some(bar) = clip(
        run_start,
        run_seconds,
        window_start,
        window_end,
        run_status,
        "Job run",
        false,
    ) {
        bars.push(bar);
    }
    bars.extend(step_bars);
}

fn clip(
    start: NaiveDateTime,
    seconds: i64,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    status: &str,
    label: &str,
    is_step: bool,
) -> Option<TimelineBar> {
    let end = start + chrono::Duration::seconds(seconds.max(1));
    if end <= window_start || start >= window_end {
        return None;
    }
    let visible_start = start.max(window_start);
    let visible_end = end.min(window_end);
    let detail = format!(
        "{label} - {status} - started {} - {}",
        start.format("%Y-%m-%d %-I:%M:%S %p"),
        format_duration(u32::try_from(seconds.max(0)).unwrap_or(u32::MAX)),
    );
    Some(TimelineBar {
        start: seconds_between(window_start, visible_start) as f32,
        length: seconds_between(visible_start, visible_end).max(1) as f32,
        status: status.to_owned(),
        label: label.to_owned(),
        detail,
        is_step,
    })
}

fn parse(raw: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(raw.trim(), SQL_TIME).ok()
}

fn seconds_between(from: NaiveDateTime, to: NaiveDateTime) -> i64 {
    (to - from).num_seconds().max(0)
}
