// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

mod server;

use server::{start_server, stop_server, get_server_status, broadcast_message_command, ServerState};


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ServerState::new()) 
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, start_server, stop_server, get_server_status, broadcast_message_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

}
