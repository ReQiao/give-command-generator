// pub mod（不是 mod）是为了 tests/ 下的集成测试能直接调用这些模块里的
// tauri::command 函数，用真实网络请求走一遍完整链路，而不是只测内部逻辑。
pub mod ai;
pub mod auth;
pub mod billing;
mod datapack;
pub mod remote;
pub mod session;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            ai::ai_generate,
            auth::auth_state,
            auth::auth_required,
            auth::auth_register_begin,
            auth::auth_register_resend,
            auth::auth_register_verify,
            auth::auth_login,
            auth::auth_logout,
            auth::auth_change_password,
            auth::auth_reset_begin,
            auth::auth_reset_confirm,
            billing::billing_state,
            billing::billing_activate,
            billing::billing_topup_tiers,
            billing::billing_recharge,
            datapack::datapack_list_saves,
            datapack::datapack_deploy,
            datapack::datapack_default_saves_dir,
            datapack::datapack_detect_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
