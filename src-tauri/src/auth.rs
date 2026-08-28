//! 账号相关的 Tauri 命令：前端 invoke 的入口，实际工作都转发给 remote.rs。
//!
//! 这一层刻意很薄，只做三件事：
//!   1. 把服务器发的 token 落到本地会话文件（前端拿不到 token 本身，见 session.rs）；
//!   2. 做一点点即时的格式校验，让明显打错的输入不用跑一趟网络就能得到反馈；
//!   3. 把服务端返回的中文错误原样透给前端——服务端的文案是精心措过辞的
//!      （比如"用户名或密码不对"对"账号不存在"和"密码错了"故意给同一句），
//!      这里不要自作主张改写，改写就可能把防枚举设计破坏掉。
//!
//! 真正的判断权全在服务器：本地校验通过不代表能注册成功，本地存着 token
//! 也不代表还登录着。

use crate::remote;
use crate::session;
use serde::Serialize;

/// 前端要展示的登录态。token 不在里面——那是故意的。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub logged_in: bool,
    pub username: String,
    pub phone_masked: String,
    pub balance: i64,
    pub activated: bool,
    /// 服务器连不上时为 true。界面要把"没登录"和"连不上服务器"分开说，
    /// 否则用户会以为自己账号出了问题，跑去反复重新注册。
    pub offline: bool,
}

impl AuthState {
    fn logged_out() -> Self {
        AuthState {
            logged_in: false,
            username: String::new(),
            phone_masked: String::new(),
            balance: 0,
            activated: false,
            offline: false,
        }
    }
}

/// 手机号本地预检。和服务端 crypto::normalize_phone 是同一套规则，两边各留一份
/// 是因为它们是两个独立部署的二进制，规则简单，保持一致比抽公共 crate 划算。
fn looks_like_phone(raw: &str) -> bool {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.strip_prefix("86").unwrap_or(&digits);
    digits.len() == 11
        && digits.starts_with('1')
        && matches!(digits.as_bytes()[1], b'3'..=b'9')
}

/// 启动时问一次"我现在算登录着吗"。
///
/// 不看本地文件里的 expires_at 就直接下结论——本地时钟不准是常事，而且
/// 服务器可能因为改密码/管理员操作提前吊销了会话。以服务器的回答为准。
#[tauri::command]
pub async fn auth_state() -> Result<AuthState, ()> {
    if session::token().is_none() {
        return Ok(AuthState::logged_out());
    }
    match remote::me().await {
        Ok(me) => Ok(AuthState {
            logged_in: true,
            username: me.user.username,
            phone_masked: me.user.phone_masked,
            balance: me.balance,
            activated: me.activated,
            offline: false,
        }),
        Err(e) => {
            // remote::parse_json 遇到 401 已经清过本地会话了。这里只需要区分
            // "服务器说你没登录" 和 "根本没连上服务器"。
            let offline = e.starts_with("无法连接服务器");
            Ok(AuthState { offline, ..AuthState::logged_out() })
        }
    }
}

/// 服务端的登录门禁开关。取不到（服务器不可达）时默认**不要求**登录——
/// 宁可放行也不要把用户锁死在一个连不上服务器的门禁后面：反正真去调
/// /v1/ai/generate 还是会被服务端 401 挡住，安全性不受影响，
/// 但用户至少能看清"是服务器连不上"而不是对着一个打不开的登录框发呆。
#[tauri::command]
pub async fn auth_required() -> Result<bool, ()> {
    Ok(remote::server_version().await.map(|v| v.auth_required).unwrap_or(false))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CodeSent {
    pub phone_masked: String,
    pub expires_in_secs: u32,
    pub log_mode: bool,
}

#[tauri::command]
pub async fn auth_register_begin(
    username: String,
    password: String,
    confirm_password: String,
    phone: String,
) -> Result<CodeSent, String> {
    // "两次密码不一致"完全可以在本地判断，没必要跑一趟网络
    if password != confirm_password {
        return Err("两次输入的密码不一样。".to_string());
    }
    if !looks_like_phone(&phone) {
        return Err("请输入 11 位中国大陆手机号。".to_string());
    }
    let r = remote::register_begin(username.trim(), &password, phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
    })
}

#[tauri::command]
pub async fn auth_register_resend(phone: String) -> Result<CodeSent, String> {
    let r = remote::register_resend(phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
    })
}

#[tauri::command]
pub async fn auth_register_verify(phone: String, code: String) -> Result<AuthState, String> {
    let s = remote::register_verify(phone.trim(), code.trim()).await?;
    session::save(&s.token, s.expires_at);
    Ok(AuthState {
        logged_in: true,
        username: s.user.username,
        phone_masked: s.user.phone_masked,
        balance: s.balance,
        activated: s.activated,
        offline: false,
    })
}

#[tauri::command]
pub async fn auth_login(account: String, password: String) -> Result<AuthState, String> {
    let s = remote::login(account.trim(), &password).await?;
    session::save(&s.token, s.expires_at);
    Ok(AuthState {
        logged_in: true,
        username: s.user.username,
        phone_masked: s.user.phone_masked,
        balance: s.balance,
        activated: s.activated,
        offline: false,
    })
}

#[tauri::command]
pub async fn auth_logout() -> Result<(), String> {
    // remote::logout 内部无论成败都会清本地会话
    let _ = remote::logout().await;
    Ok(())
}

#[tauri::command]
pub async fn auth_change_password(
    old_password: String,
    new_password: String,
    confirm_password: String,
) -> Result<(), String> {
    if new_password != confirm_password {
        return Err("两次输入的新密码不一样。".to_string());
    }
    remote::change_password(&old_password, &new_password).await
}

#[tauri::command]
pub async fn auth_reset_begin(phone: String) -> Result<CodeSent, String> {
    if !looks_like_phone(&phone) {
        return Err("请输入 11 位中国大陆手机号。".to_string());
    }
    let r = remote::reset_begin(phone.trim()).await?;
    Ok(CodeSent {
        phone_masked: r.phone_masked,
        expires_in_secs: r.expires_in_secs,
        log_mode: r.log_mode,
    })
}

#[tauri::command]
pub async fn auth_reset_confirm(
    phone: String,
    code: String,
    new_password: String,
    confirm_password: String,
) -> Result<(), String> {
    if new_password != confirm_password {
        return Err("两次输入的密码不一样。".to_string());
    }
    remote::reset_confirm(phone.trim(), code.trim(), &new_password).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_precheck_matches_server_rules() {
        assert!(looks_like_phone("13800138000"));
        assert!(looks_like_phone("+86 138 0013 8000"));
        assert!(looks_like_phone("138-0013-8000"));
        assert!(!looks_like_phone("12800138000"), "第二位是 2 不是合法号段");
        assert!(!looks_like_phone("1380013800"), "10 位");
        assert!(!looks_like_phone(""));
        assert!(!looks_like_phone("abcdefghijk"));
    }
}
