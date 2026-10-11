#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    sql_job_monitor::app::Application::new()?.run()
}
