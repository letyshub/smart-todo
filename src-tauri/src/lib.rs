mod commands;
mod db;
mod sync;

use commands::sync::SyncState;
use commands::timer::TimerState;
use db::DbState;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// How often the sync folder is checked in the background.
///
/// Cloud clients take seconds to minutes to move a file anyway, so polling more
/// often would only spin the disk without the other machine's changes being
/// there any sooner.
const SYNC_INTERVAL: Duration = Duration::from_secs(30);

/// One background sync pass. Errors are reported to the UI rather than thrown:
/// a sync folder that is briefly unavailable is normal, and the next pass will
/// pick up whatever was missed.
fn sync_pass(app: &tauri::AppHandle) {
    let (Some(db), Some(state)) = (app.try_state::<DbState>(), app.try_state::<SyncState>()) else {
        return;
    };
    let folder = match state.0.lock() {
        Ok(guard) => guard.as_ref().map(sync::store::SyncFolder::new),
        Err(_) => return,
    };
    let Some(folder) = folder else { return };
    let Ok(conn) = db.0.lock() else { return };

    match sync::run(&conn, &folder) {
        Ok(report) => {
            // Only wake the UI when something actually moved.
            if report.applied > 0 || report.conflicts > 0 {
                let _ = app.emit("sync:changed", report);
            }
        }
        Err(e) => {
            let _ = app.emit("sync:error", e.to_string());
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let startup = db::plan_startup();
    if let Some(parent) = startup.db_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let conn = db::open(&startup.db_path.to_string_lossy()).expect("failed to open database");

    // A folder an earlier version used to host the database in becomes the sync
    // folder, and this machine's data is published into it.
    let sync_dir = match &startup.adopt_sync_dir {
        Some(dir) => match sync::adopt_folder(&conn, &db::config_dir(), dir) {
            Ok(_) => Some(dir.clone()),
            Err(e) => {
                eprintln!("could not adopt {} as the sync folder: {e}", dir.display());
                None
            }
        },
        None => sync::read_folder(&db::config_dir()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(DbState(Mutex::new(conn)))
        .manage(TimerState(Mutex::new(HashMap::new())))
        .manage(SyncState(Mutex::new(sync_dir)))
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                sync_pass(&handle);
                loop {
                    std::thread::sleep(SYNC_INTERVAL);
                    sync_pass(&handle);
                }
            });
            Ok(())
        })
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
            commands::stats::get_productivity_stats,
            commands::sync::get_sync_status,
            commands::sync::set_sync_folder,
            commands::sync::disable_sync,
            commands::sync::sync_now,
            commands::sync::get_conflicts,
            commands::sync::resolve_conflict,
            commands::sync::set_device_name,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
