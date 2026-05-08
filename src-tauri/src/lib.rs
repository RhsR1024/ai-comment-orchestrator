pub mod commenter;

#[cfg(not(test))]
use std::path::PathBuf;
#[cfg(not(test))]
use tauri::Manager;

#[cfg(not(test))]
fn commenter_data_root(app: &tauri::App) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root)
}

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = commenter_data_root(app)
                .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
            app.manage(commenter::commands::CommenterCommandSurface::new(data_root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commenter::commands::commenter_upsert_project_profile,
            commenter::commands::commenter_list_project_profiles,
            commenter::commands::commenter_enqueue_run,
            commenter::commands::commenter_list_runs,
            commenter::commands::commenter_get_run_detail,
            commenter::commands::commenter_delete_run,
            commenter::commands::commenter_start_run,
            commenter::commands::commenter_pause_run,
            commenter::commands::commenter_resume_run,
            commenter::commands::commenter_cancel_run,
            commenter::commands::commenter_list_review_jobs,
            commenter::commands::commenter_accept_review_job,
            commenter::commands::commenter_reject_review_job,
            commenter::commands::commenter_retry_job,
            commenter::commands::commenter_open_external_diff,
            commenter::commands::commenter_rollback_run,
            commenter::commands::commenter_get_app_settings,
            commenter::commands::commenter_update_app_settings,
            commenter::commands::commenter_get_diff_tool_settings,
            commenter::commands::commenter_update_diff_tool_settings,
            commenter::commands::commenter_list_dir,
            commenter::commands::commenter_get_candidate_text,
            commenter::commands::commenter_get_data_paths,
            commenter::commands::commenter_open_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
