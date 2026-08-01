mod ai;
mod billing;
mod datapack;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(billing::Billing::default())
        .invoke_handler(tauri::generate_handler![
            ai::ai_generate,
            billing::billing_state,
            billing::billing_activate,
            datapack::datapack_list_saves,
            datapack::datapack_deploy,
            datapack::datapack_default_saves_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
