mod db;
mod commands;

use db::DbState;
use commands::timer::TimerState;
use std::sync::Mutex;
use std::collections::HashMap;

pub struct DbPath(pub String);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("smart-todo");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("data.db").to_string_lossy().to_string();
    let conn = db::open(&db_path).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(conn)))
        .manage(TimerState(Mutex::new(HashMap::new())))
        .manage(DbPath(db_path))
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
            commands::timer::start_timer,
            commands::timer::stop_timer,
            commands::timer::get_active_timers,
            commands::timer::get_timer_sessions,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::change_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
