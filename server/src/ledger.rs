//! 账本：按 device_id 记录余额 / 激活状态。
//!
//! 和客户端 billing.rs 的本地文件不同——这份数据活在服务器上，客户端完全碰不到，
//! 所以不需要像客户端那样加 HMAC 签名防篡改（那是为了防"用户改自己电脑上的文件"，
//! 这里的威胁模型是"服务器本身被入侵"，签名在那种情况下毫无意义，真正该做的是
//! 服务器自身的安全加固——SSH 只允许密钥登录、防火墙只开必要端口，这些不是本
//! 模块能管的事）。
//!
//! device_id 由客户端生成一个随机 UUID 并本地持久化，不是登录账号——这意味着
//! 卸载重装能刷新一次首次体验额度，这是刻意接受的权衡（做真正的账号体系是
//! 更大的工程，现在没必要）。这份实现真正解决的问题是两个：
//!   1. 大模型 key 只活在这台服务器的环境变量里，从此不再跟着客户端分发；
//!   2. 激活码在全局范围内唯一——本地版只能防"这台设备兑过"，这里能防
//!      "同一个码被转发给好几个人各自兑换"。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// 首次见到某个 device_id 时发放的体验额度，数值和客户端此前的约定保持一致。
const WELCOME_BALANCE: i64 = 9999;

/// 激活码兑换赠送的灵魂币，和客户端 billing.rs 的约定保持一致。
const ACTIVATE_BONUS: i64 = 100;

/// 充值档位：(人民币元, 灵魂币)。和客户端 billing.rs::TOPUP_TIERS 是同一份表——
/// 服务器成为权威之后，这张表就该只在这一处维护，客户端那份以后应该改成
/// 从服务器接口拉取展示，而不是各自维护一份、改一边忘了改另一边。
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

/// device_id 的合理长度上限——纯粹防止有人拿一个巨大的字符串当 device_id
/// 灌进账本文件，不是什么严密的安全边界，就是个廉价的健壮性检查。
const MAX_DEVICE_ID_LEN: usize = 128;

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub balance: i64,
    pub activated: bool,
    #[serde(default)]
    pub redeemed_keys: Vec<String>,
}

impl Account {
    fn fresh() -> Self {
        Account { balance: WELCOME_BALANCE, activated: false, redeemed_keys: Vec::new() }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedLedger {
    accounts: HashMap<String, Account>,
}

pub struct Ledger {
    accounts: Mutex<HashMap<String, Account>>,
    /// None 表示不落盘（纯内存，测试用）。
    path: Option<PathBuf>,
}

pub fn is_valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_DEVICE_ID_LEN
}

/// 校验 SOUL-XXXX-XXXX-XXXX 格式（大小写不敏感），和客户端 billing.rs 的
/// is_valid_license 是同一套规则——两边各自维护是因为它们是两个独立部署的
/// 二进制，不方便共享代码，规则简单，保持一致比抽公共 crate 划算。
fn is_valid_license(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    parts.len() == 4
        && parts[0].eq_ignore_ascii_case("SOUL")
        && parts[1..]
            .iter()
            .all(|seg| seg.len() == 4 && seg.chars().all(|c| c.is_ascii_alphanumeric()))
}

impl Ledger {
    pub fn load_or_default(path: PathBuf) -> Self {
        let accounts = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<PersistedLedger>(&text).ok())
            .map(|p| p.accounts)
            .unwrap_or_default();
        Ledger { accounts: Mutex::new(accounts), path: Some(path) }
    }

    #[cfg(test)]
    fn in_memory() -> Self {
        Ledger { accounts: Mutex::new(HashMap::new()), path: None }
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        let accounts = self.accounts.lock().unwrap().clone();
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::error!("创建账本目录失败：{e}");
                return;
            }
        }
        match serde_json::to_string_pretty(&PersistedLedger { accounts }) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    tracing::error!("写入账本失败：{e}");
                }
            }
            Err(e) => tracing::error!("序列化账本失败：{e}"),
        }
    }

    /// 取账户快照；第一次见到这个 device_id 就顺手创建并发首次体验额度。
    pub fn snapshot(&self, device_id: &str) -> Account {
        let is_new = {
            let mut accounts = self.accounts.lock().unwrap();
            let existed = accounts.contains_key(device_id);
            accounts.entry(device_id.to_string()).or_insert_with(Account::fresh);
            !existed
        };
        if is_new {
            self.persist(); // 只在真的新建时落盘，纯查询不用每次都写文件
        }
        self.accounts.lock().unwrap().get(device_id).cloned().unwrap_or_default()
    }

    pub fn consume(&self, device_id: &str, coins: i64) -> Account {
        let acc = {
            let mut accounts = self.accounts.lock().unwrap();
            let acc = accounts.entry(device_id.to_string()).or_insert_with(Account::fresh);
            acc.balance = (acc.balance - coins.max(0)).max(0);
            acc.clone()
        };
        self.persist();
        acc
    }

    pub fn add(&self, device_id: &str, coins: i64) -> Account {
        let acc = {
            let mut accounts = self.accounts.lock().unwrap();
            let acc = accounts.entry(device_id.to_string()).or_insert_with(Account::fresh);
            acc.balance += coins.max(0);
            acc.clone()
        };
        self.persist();
        acc
    }

    /// 充值：只接受预设档位（和客户端此前的校验逻辑一致，见 billing.rs::recharge）。
    pub fn topup(&self, device_id: &str, coins: i64) -> Result<Account, String> {
        if !TOPUP_TIERS.iter().any(|&(_, tier_coins)| tier_coins == coins) {
            return Err("不是预设的充值档位。".to_string());
        }
        Ok(self.add(device_id, coins))
    }

    /// 激活码兑换。跟客户端版本最大的区别：这里的"是否已兑换"是**全局**判断，
    /// 不是只查这一个 device_id 自己的记录——防的是"一个码被转发给好几个人
    /// 分别在自己设备上兑换"，这件事本地版从架构上就做不到。
    pub fn activate(&self, device_id: &str, license_key: &str) -> Result<Account, String> {
        let key = license_key.trim().to_string();
        if !is_valid_license(&key) {
            return Err("激活码格式无效（示例：SOUL-AB12-CD34-EF56）。".to_string());
        }
        let acc = {
            let mut accounts = self.accounts.lock().unwrap();
            let already_used = accounts
                .values()
                .any(|a| a.redeemed_keys.iter().any(|k| k.eq_ignore_ascii_case(&key)));
            if already_used {
                return Err("这个激活码已经被使用过了。".to_string());
            }
            let acc = accounts.entry(device_id.to_string()).or_insert_with(Account::fresh);
            acc.activated = true;
            acc.balance += ACTIVATE_BONUS;
            acc.redeemed_keys.push(key);
            acc.clone()
        };
        self.persist();
        Ok(acc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_device_gets_welcome_balance() {
        let ledger = Ledger::in_memory();
        let acc = ledger.snapshot("device-a");
        assert_eq!(acc.balance, WELCOME_BALANCE);
        assert!(!acc.activated);
    }

    #[test]
    fn consume_floors_at_zero_and_persists_across_calls() {
        let ledger = Ledger::in_memory();
        ledger.snapshot("device-a");
        assert_eq!(ledger.consume("device-a", 4000).balance, WELCOME_BALANCE - 4000);
        assert_eq!(ledger.consume("device-a", 999_999).balance, 0, "余额不该变负数");
    }

    #[test]
    fn activate_adds_bonus_not_overwrites() {
        let ledger = Ledger::in_memory();
        ledger.snapshot("device-a"); // 先建号，拿到 WELCOME_BALANCE
        ledger.consume("device-a", WELCOME_BALANCE - 5000); // 花掉一些，剩 5000
        let acc = ledger.activate("device-a", "SOUL-AB12-CD34-EF56").unwrap();
        assert_eq!(acc.balance, 5000 + ACTIVATE_BONUS, "激活应该叠加而不是覆盖已有余额");
        assert!(acc.activated);
    }

    #[test]
    fn same_license_cannot_be_used_by_a_different_device() {
        // 这是服务器版真正解决的问题：本地版做不到"跨设备"防重复，这里能做到。
        let ledger = Ledger::in_memory();
        ledger.activate("device-a", "SOUL-AB12-CD34-EF56").unwrap();
        let err = ledger.activate("device-b", "SOUL-AB12-CD34-EF56");
        assert!(err.is_err(), "同一个码不该能被另一台设备再兑一次");
    }

    #[test]
    fn same_license_case_insensitive_dedup() {
        let ledger = Ledger::in_memory();
        ledger.activate("device-a", "SOUL-AB12-CD34-EF56").unwrap();
        assert!(ledger.activate("device-b", "soul-ab12-cd34-ef56").is_err());
    }

    #[test]
    fn rejects_malformed_license_without_touching_balance() {
        let ledger = Ledger::in_memory();
        let before = ledger.snapshot("device-a").balance;
        assert!(ledger.activate("device-a", "NOPE").is_err());
        assert_eq!(ledger.snapshot("device-a").balance, before);
    }

    #[test]
    fn topup_accepts_only_preset_tiers() {
        let ledger = Ledger::in_memory();
        ledger.snapshot("device-a");
        assert!(ledger.topup("device-a", 999).is_err());
        assert_eq!(ledger.topup("device-a", 1000).unwrap().balance, WELCOME_BALANCE + 1000);
    }

    #[test]
    fn balance_survives_restart_via_file() {
        let dir = std::env::temp_dir().join("soul-ledger-test-restart");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ledger.json");

        let ledger = Ledger::load_or_default(path.clone());
        ledger.consume("device-a", 1000);

        // 模拟重启：重新从磁盘加载
        let reloaded = Ledger::load_or_default(path);
        assert_eq!(reloaded.snapshot("device-a").balance, WELCOME_BALANCE - 1000);
    }

    #[test]
    fn device_id_validation() {
        assert!(is_valid_device_id("a-normal-uuid"));
        assert!(!is_valid_device_id(""));
        assert!(!is_valid_device_id(&"x".repeat(200)));
    }
}
