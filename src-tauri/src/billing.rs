//! 计费 / 账号验证（骨架）。
//!
//! 变现设想：客户端内置激活码或登录，每次 AI 调用扣一次余额（充值制）。
//! 收费逻辑放在 Rust 后端而不进 webview，避免前端被改。
//!
//! 现状：这是**占位骨架**——默认即为已激活、余额充足，激活码只做格式校验，
//! 没有任何服务端校验，也不做本地持久化（重启即回到默认值）。
//! 目的是先把接口形状定下来，等真要变现时再接发卡/账号服务端，
//! 届时只需替换 `billing_activate` 内部实现与 `AuthState` 的来源。

use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub activated: bool,
    pub license_key: Option<String>,
    /// 剩余可用调用次数（充值制余额）。
    pub balance: i64,
}

impl Default for AuthState {
    fn default() -> Self {
        // 占位：默认放行，不给用户设门槛。真实变现时应改为 activated: false, balance: 0。
        AuthState {
            activated: true,
            license_key: Some("builtin".to_string()),
            balance: 9999,
        }
    }
}

/// Tauri 托管的全局计费状态。
#[derive(Default)]
pub struct Billing(pub Mutex<AuthState>);

impl Billing {
    /// 当前余额快照。
    pub fn balance(&self) -> i64 {
        self.0.lock().map(|st| st.balance).unwrap_or(0)
    }

    /// 扣减一次余额，返回扣减后的余额（最低 0）。
    pub fn consume(&self) -> i64 {
        match self.0.lock() {
            Ok(mut st) => {
                st.balance = (st.balance - 1).max(0);
                st.balance
            }
            Err(_) => 0,
        }
    }
}

/// 读取账号 / 余额状态。
#[tauri::command]
pub fn billing_state(billing: tauri::State<'_, Billing>) -> AuthState {
    billing.0.lock().map(|st| st.clone()).unwrap_or_default()
}

/// 校验激活码（骨架：仅格式校验 + 占位余额）。
#[tauri::command]
pub fn billing_activate(
    license_key: String,
    billing: tauri::State<'_, Billing>,
) -> Result<AuthState, String> {
    let key = license_key.trim().to_string();
    if !is_valid_license(&key) {
        return Err("激活码格式无效（示例：SOUL-AB12-CD34-EF56）。".to_string());
    }
    let mut st = billing.0.lock().map_err(|_| "计费状态锁已损坏".to_string())?;
    st.activated = true;
    st.license_key = Some(key);
    st.balance = 100; // 占位：激活赠送 100 次
    Ok(st.clone())
}

/// 校验 SOUL-XXXX-XXXX-XXXX 格式（大小写不敏感）。
fn is_valid_license(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    parts.len() == 4
        && parts[0].eq_ignore_ascii_case("SOUL")
        && parts[1..]
            .iter()
            .all(|seg| seg.len() == 4 && seg.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_license() {
        assert!(is_valid_license("SOUL-AB12-CD34-EF56"));
        assert!(is_valid_license("soul-ab12-cd34-ef56"));
    }

    #[test]
    fn rejects_malformed_license() {
        assert!(!is_valid_license(""));
        assert!(!is_valid_license("SOUL-AB12-CD34"));           // 段数不足
        assert!(!is_valid_license("MCAI-AB12-CD34-EF56"));      // 前缀不符
        assert!(!is_valid_license("SOUL-AB1-CD34-EF56"));       // 段长不足
        assert!(!is_valid_license("SOUL-AB1@-CD34-EF56"));      // 非字母数字
    }

    #[test]
    fn consume_decrements_and_floors_at_zero() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 2;
        }
        assert_eq!(billing.consume(), 1);
        assert_eq!(billing.consume(), 0);
        assert_eq!(billing.consume(), 0, "余额不应变成负数");
    }
}
