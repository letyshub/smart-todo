mod db;
mod commands;

use db::DbState;
use commands::timer::TimerState;
use std::sync::Mutex;
use std::collections::HashMap;

/// Path of the currently open database file. Kept in a Mutex because
/// `change_data_dir` swaps the connection at runtime.
pub struct DbPath(pub Mutex<String>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_file = db::resolve_db_path();
    if let Some(parent) = db_file.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let db_path = db_file.to_string_lossy().to_string();
    let conn = db::open(&db_path).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(DbState(Mutex::new(conn)))
        .manage(TimerState(Mutex::new(HashMap::new())))
        .manage(DbPath(Mutex::new(db_path)))
        .invoke_handler(tauri::generate_handler![
            commands::lists::get_lists,
            commands::lists::create_list,
            commands::lists::update_list,
            commands::lists::delete_list,
            commands::tasks::get_tasks,
            commands::tasks::create_task,
            commands::tasks::update_task,
            commands::tasks::delete_task,
            commands::tasks::get_dashboard_tasks,
            commands::tasks::set_task_tags,
            commands::tasks::get_all_tags,
            commands::tasks::create_subtask,
            commands::tasks::get_subtasks,
            commands::tasks::create_child_task,
            commands::tasks::get_child_tasks,
            commands::timer::start_timer,
            commands::timer::stop_timer,
            commands::timer::get_active_timers,
            commands::timer::get_timer_sessions,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::change_data_dir,
            commands::stats::get_productivity_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
