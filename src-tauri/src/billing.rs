//! 计费 / 账号验证（骨架）。
//!
//! 变现设想：客户端内置激活码或登录，每次 AI 调用按真实 token 用量折算扣费
//! （单位"灵魂币"，换算逻辑在 ai.rs 的 coins_to_charge）。收费逻辑放在 Rust
//! 后端而不进 webview，避免前端被改。
//!
//! 现状：这是**占位骨架**——默认即为已激活、余额充足，激活码只做格式校验，
//! 充值（billing_recharge）现在是免费直接加余额，没有接真实支付网关，
//! 也没有任何服务端校验，也不做本地持久化（重启即回到默认值）。
//! 目的是先把接口形状、充值档位、扣费逻辑都定下来，等真要变现时再接
//! 支付网关/发卡/账号服务端，届时只需替换 `billing_recharge`/`billing_activate`
//! 内部实现与 `AuthState` 的来源，前端调用方式不用变。

use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub activated: bool,
    pub license_key: Option<String>,
    /// 剩余灵魂币余额。
    pub balance: i64,
    /// 已经兑换过的激活码，防止同一个码反复点着加钱。
    #[serde(default)]
    pub redeemed_keys: Vec<String>,
}

impl Default for AuthState {
    fn default() -> Self {
        // 占位：默认放行，不给用户设门槛。真实变现时应改为 activated: false, balance: 0。
        AuthState {
            activated: true,
            license_key: Some("builtin".to_string()),
            balance: 9999,
            redeemed_keys: Vec::new(),
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

    /// 扣减指定数量的灵魂币，返回扣减后的余额（最低 0）。
    pub fn consume(&self, coins: i64) -> i64 {
        match self.0.lock() {
            Ok(mut st) => {
                st.balance = (st.balance - coins.max(0)).max(0);
                st.balance
            }
            Err(_) => 0,
        }
    }

    /// 增加余额，返回增加后的余额。当前是免费加余额（充值功能还没接真实支付），
    /// 真要收钱时应该把校验/入账逻辑挪到有支付回调可信来源的地方，而不是直接信前端传参。
    pub fn add(&self, coins: i64) -> i64 {
        match self.0.lock() {
            Ok(mut st) => {
                st.balance += coins.max(0);
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

/// 充值档位：(人民币元, 灵魂币)。数字来自和用户对过的兑换表——越大档位单价越
/// 划算，鼓励一次充多一点。前后端共用同一份，前端展示用的档位从这里读，
/// 不在 Vue 里重复写一遍数字，免得改一边忘了改另一边。
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopupTier {
    pub yuan: f64,
    pub coins: i64,
}

/// 给前端展示充值档位用。
#[tauri::command]
pub fn billing_topup_tiers() -> Vec<TopupTier> {
    TOPUP_TIERS.iter().map(|&(yuan, coins)| TopupTier { yuan, coins }).collect()
}

/// 充值逻辑本体，和 tauri::State 解耦方便单测（State 不好在测试里手动构造，
/// 参考 is_valid_license 跟 billing_activate 分离的同一个理由）。
fn recharge(billing: &Billing, coins: i64) -> Result<AuthState, String> {
    if !TOPUP_TIERS.iter().any(|&(_, tier_coins)| tier_coins == coins) {
        return Err("不是预设的充值档位。".to_string());
    }
    billing.add(coins);
    Ok(billing.0.lock().map(|st| st.clone()).unwrap_or_default())
}

/// 充值。现在还没接真实支付网关，点了档位直接免费加到余额——只接受预设档位
/// 的灵魂币数量，防止意外传进一个乱七八糟的数字（哪怕现在免费，这个校验
/// 也是以后接真实支付时该有的基本检查，先写上不用等以后补）。
#[tauri::command]
pub fn billing_recharge(coins: i64, billing: tauri::State<'_, Billing>) -> Result<AuthState, String> {
    recharge(&billing, coins)
}

/// 激活码兑换赠送的灵魂币。
const ACTIVATE_BONUS: i64 = 100;

/// 激活逻辑本体，和 tauri::State 解耦方便单测（同 recharge/is_valid_license 的理由）。
fn activate(billing: &Billing, license_key: &str) -> Result<AuthState, String> {
    let key = license_key.trim().to_string();
    if !is_valid_license(&key) {
        return Err("激活码格式无效（示例：SOUL-AB12-CD34-EF56）。".to_string());
    }
    let mut st = billing.0.lock().map_err(|_| "计费状态锁已损坏".to_string())?;

    // 同一个码只能兑一次。这拦不住把配置文件删了重来的人（真正的一次性核销要么
    // 靠服务器记录、要么靠签名码），但至少不会让同一个码在界面上点一次加一次。
    if st.redeemed_keys.iter().any(|k| k.eq_ignore_ascii_case(&key)) {
        return Err("这个激活码已经用过了。".to_string());
    }

    st.activated = true;
    st.license_key = Some(key.clone());
    st.redeemed_keys.push(key);
    // 必须是叠加不是覆盖——先充值再激活的用户，之前这里会把余额直接清成 100。
    st.balance += ACTIVATE_BONUS;
    Ok(st.clone())
}

/// 校验激活码（骨架：仅格式校验 + 赠送固定额度）。
#[tauri::command]
pub fn billing_activate(
    license_key: String,
    billing: tauri::State<'_, Billing>,
) -> Result<AuthState, String> {
    activate(&billing, &license_key)
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
    fn consume_decrements_by_given_amount_and_floors_at_zero() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 100;
        }
        assert_eq!(billing.consume(30), 70);
        assert_eq!(billing.consume(1000), 0, "余额不应变成负数");
    }

    #[test]
    fn add_increments_balance() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 0;
        }
        assert_eq!(billing.add(500), 500);
        assert_eq!(billing.add(300), 800);
    }

    #[test]
    fn recharge_accepts_only_preset_tiers() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 0;
        }
        let ok = recharge(&billing, 1000).unwrap();
        assert_eq!(ok.balance, 1000);
        assert!(recharge(&billing, 999).is_err(), "非预设档位应该被拒绝");
    }

    #[test]
    fn activate_adds_bonus_instead_of_overwriting_balance() {
        // 曾经的真 bug：这里是 st.balance = 100 直接赋值，先充值再激活会把余额清光。
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 5000;
        }
        let st = activate(&billing, "SOUL-AB12-CD34-EF56").unwrap();
        assert_eq!(st.balance, 5000 + ACTIVATE_BONUS, "激活应该叠加而不是覆盖已有余额");
        assert!(st.activated);
    }

    #[test]
    fn same_license_cannot_be_redeemed_twice() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 0;
        }
        assert_eq!(activate(&billing, "SOUL-AB12-CD34-EF56").unwrap().balance, ACTIVATE_BONUS);
        // 大小写不同也算同一个码
        let again = activate(&billing, "soul-ab12-cd34-ef56");
        assert!(again.is_err(), "同一个码不该能兑第二次");
        assert_eq!(billing.balance(), ACTIVATE_BONUS, "被拒的兑换不该改动余额");
    }

    #[test]
    fn activate_rejects_bad_format_without_touching_balance() {
        let billing = Billing::default();
        {
            let mut st = billing.0.lock().unwrap();
            st.balance = 42;
        }
        assert!(activate(&billing, "NOPE").is_err());
        assert_eq!(billing.balance(), 42);
    }

    #[test]
    fn topup_tiers_match_agreed_table() {
        let tiers = billing_topup_tiers();
        assert_eq!(tiers.len(), 8);
        assert_eq!((tiers[0].yuan, tiers[0].coins), (1.0, 1000));
        assert_eq!((tiers[7].yuan, tiers[7].coins), (648.0, 888888));
    }
}
