//! 计费 / 账号：转发到自建服务器（仓库根目录 server/）。
//!
//! 这个文件曾经是本地状态的骨架——余额存在本机一份 JSON 文件里，先后加过
//! HMAC 签名防手改、落盘持久化。这些都只能"抬高篡改门槛"，治标不治本：
//! 用户对自己电脑上的文件有完全读写权限，任何本地校验都绕不开这个前提。
//! 现在余额/激活码的权威数据完全搬到了服务器（server/src/ledger.rs），
//! 本地这份代码从此不保存、也不判断任何余额数字——每次都问服务器，
//! 服务器的回答就是唯一作数的答案。本地signature那套已经被这个更彻底的
//! 方案取代，不是"多一层"，是"整个换掉"。
//!
//! 激活码的核对逻辑也因此变强了：本地版本只能防"这台设备重复兑换"，
//! 服务器版本能防"同一个码被转发给好几个人各自在自己设备上兑换"——
//! 这件事本地架构上就做不到，见 server/src/ledger.rs::activate 的注释。

use crate::device;
use crate::remote::{self, AccountView};

/// 激活码格式：SOUL-XXXX-XXXX-XXXX（大小写不敏感）。真正是否已被使用由
/// 服务器判断，这里只做前端友好的即时格式提示，避免明显打错格式的输入
/// 也要跑一趟网络才能得到"格式不对"的反馈。
fn is_valid_license_format(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    parts.len() == 4
        && parts[0].eq_ignore_ascii_case("SOUL")
        && parts[1..]
            .iter()
            .all(|seg| seg.len() == 4 && seg.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// 读取账号 / 余额状态。
#[tauri::command]
pub async fn billing_state() -> Result<AccountView, String> {
    remote::balance(&device::get_or_create()).await
}

/// 充值档位：(人民币元, 灵魂币)。和服务器 server/src/ledger.rs::TOPUP_TIERS
/// 是同一份表，理想情况下应该只在服务器维护一份、客户端展示时向服务器要，
/// 现在先各自保留一份静态数据，改一边记得改另一边。
pub const TOPUP_TIERS: &[(f64, i64)] = &[
    (1.0, 1000),
    (5.0, 5100),
    (10.0, 12000),
    (30.0, 33333),
    (66.0, 70000),
    (128.0, 166666),
    (288.0, 333333),
    (648.0, 888888),
];

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopupTier {
    pub yuan: f64,
    pub coins: i64,
}

/// 给前端展示充值档位用，纯静态数据，不需要联网。
#[tauri::command]
pub fn billing_topup_tiers() -> Vec<TopupTier> {
    TOPUP_TIERS.iter().map(|&(yuan, coins)| TopupTier { yuan, coins }).collect()
}

/// 充值。现在还没接真实支付网关，服务器那边点了预设档位直接免费加到余额。
#[tauri::command]
pub async fn billing_recharge(coins: i64) -> Result<AccountView, String> {
    remote::topup(&device::get_or_create(), coins).await
}

/// 激活码兑换。真正的一次性核销、全局唯一判断都在服务器上（见
/// server/src/ledger.rs::activate），本地只做格式的前置校验。
#[tauri::command]
pub async fn billing_activate(license_key: String) -> Result<AccountView, String> {
    let key = license_key.trim();
    if !is_valid_license_format(key) {
        return Err("激活码格式无效（示例：SOUL-AB12-CD34-EF56）。".to_string());
    }
    remote::activate(&device::get_or_create(), key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_license() {
        assert!(is_valid_license_format("SOUL-AB12-CD34-EF56"));
        assert!(is_valid_license_format("soul-ab12-cd34-ef56"));
    }

    #[test]
    fn rejects_malformed_license() {
        assert!(!is_valid_license_format(""));
        assert!(!is_valid_license_format("SOUL-AB12-CD34")); // 段数不足
        assert!(!is_valid_license_format("MCAI-AB12-CD34-EF56")); // 前缀不符
        assert!(!is_valid_license_format("SOUL-AB1-CD34-EF56")); // 段长不足
        assert!(!is_valid_license_format("SOUL-AB1@-CD34-EF56")); // 非字母数字
    }

    #[test]
    fn topup_tiers_match_agreed_table() {
        let tiers = billing_topup_tiers();
        assert_eq!(tiers.len(), 8);
        assert_eq!((tiers[0].yuan, tiers[0].coins), (1.0, 1000));
        assert_eq!((tiers[7].yuan, tiers[7].coins), (648.0, 888888));
    }
}
