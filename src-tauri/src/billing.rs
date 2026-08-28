//! 计费：转发到自建服务器（仓库根目录 server/）。
//!
//! 这个文件曾经是本地状态的骨架——余额存在本机一份 JSON 文件里，先后加过
//! HMAC 签名防手改、落盘持久化。那些都只能"抬高篡改门槛"，治标不治本：
//! 用户对自己电脑上的文件有完全读写权限，任何本地校验都绕不开这个前提。
//! 现在余额/激活码的权威数据完全在服务器（server/src/ledger.rs），
//! 本地这份代码不保存、也不判断任何余额数字。
//!
//! 这一版又去掉了 device_id：身份改成真正的账号（见 auth.rs / session.rs）。
//! 老的 device_id 除了不能跨设备找回余额，还有个更糟的性质——服务端账本
//! 每个写入口都会给没见过的 key 凭空发欢迎币，删掉本地那个文件就能无限刷。

use crate::remote::{self, AccountView, TopupTier};

/// 激活码格式：SOUL-XXXX-XXXX-XXXX。
///
/// 只做"长得像不像"的即时提示，**不判断有效性**——真正的校验位是服务端用
/// 只存在于它环境变量里的 pepper 算的 HMAC（见 server/src/crypto.rs::verify_license），
/// 客户端既算不出也不该算得出，否则伪造激活码的能力就跟着客户端分发出去了。
fn looks_like_license(key: &str) -> bool {
    let flat: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    flat.len() == 16 && flat[..4].eq_ignore_ascii_case("SOUL")
}

/// 读余额 / 激活状态。未登录时服务器会 401，remote 层会顺手清掉本地会话。
#[tauri::command]
pub async fn billing_state() -> Result<AccountView, String> {
    remote::balance().await
}

/// 充值档位。改成向服务器要，不再客户端自己维护一份静态表——
/// 以前两边各存一份，改一边忘另一边就会出现"界面写着 5 元 5100 币、
/// 点下去服务器说这不是预设档位"。
#[tauri::command]
pub async fn billing_topup_tiers() -> Result<Vec<TopupTier>, String> {
    remote::topup_tiers().await
}

/// 充值。现在还没接真实支付网关，服务器那边点了预设档位直接免费加到余额，
/// 但至少要求登录 + 限流 + 记审计日志了（改造前它是完全无鉴权的）。
#[tauri::command]
pub async fn billing_recharge(coins: i64) -> Result<AccountView, String> {
    remote::topup(coins).await
}

/// 激活码兑换。一次性核销、全局唯一判断、校验位验证都在服务器上。
#[tauri::command]
pub async fn billing_activate(license_key: String) -> Result<AccountView, String> {
    let key = license_key.trim();
    if !looks_like_license(key) {
        return Err("激活码格式无效（示例：SOUL-AB2C-D3EF-GH4J）。".to_string());
    }
    remote::activate(key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_license() {
        assert!(looks_like_license("SOUL-AB2C-D3EF-GH4J"));
        assert!(looks_like_license("soul-ab2c-d3ef-gh4j"));
        assert!(looks_like_license(" SOUL AB2C D3EF GH4J "));
    }

    #[test]
    fn rejects_malformed_license() {
        assert!(!looks_like_license(""));
        assert!(!looks_like_license("SOUL-AB12-CD34"), "段数不足");
        assert!(!looks_like_license("MCAI-AB12-CD34-EF56"), "前缀不符");
    }

    #[test]
    fn client_cannot_tell_a_forged_code_from_a_real_one() {
        // 这条是刻意的：格式检查通过不代表码有效。校验位只有服务器算得出，
        // 客户端要是能判断有效性，伪造能力就跟着安装包分发出去了。
        assert!(looks_like_license("SOUL-AAAA-AAAA-AAAA"));
    }
}
