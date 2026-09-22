mod app_paths;
mod plugins;
mod process;
mod runtime;

use plugins::{
    import_plugin, list_available_plugins, list_official_plugins, open_plugin_link,
    read_plugin_config, write_plugin_config,
};
use process::{launch_bot, send_bot_command, stop_bot, ProcessState};
use runtime::{get_app_status, get_runtime_sources, install_runtime};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ProcessState::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<ProcessState>().force_kill();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            get_runtime_sources,
            install_runtime,
            list_available_plugins,
            list_official_plugins,
            open_plugin_link,
            import_plugin,
            read_plugin_config,
            write_plugin_config,
            launch_bot,
            send_bot_command,
            stop_bot
        ])
        .run(tauri::generate_context!())
        .expect("failed to run xinbot-gui-win");
}
