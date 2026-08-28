//! 账本：按**账号**记录余额 / 激活状态。
//!
//! 和改造前最大的区别：不再有 device_id 这回事。
//!
//! 老实现用客户端生成的随机 UUID（device_id）当身份，而 `snapshot`/`add`/`consume`/
//! `activate` 每一个都是 `entry(key).or_insert_with(Account::fresh)`——**任何一次
//! 对没见过的 key 的调用都会凭空发一份欢迎余额**。删掉本地那个 device_id 文件就能
//! 无限刷，这是账号体系要解决的核心问题之一。
//!
//! 现在：
//! - 账户 key 一律是 `u:<user_id>`，由 `account_key()` 统一生成，外部拿不到构造权；
//! - 欢迎余额**只在注册成功那一刻发一次**（见 auth.rs 的 `finish_registration`），
//!   本模块任何函数都不会凭空建账户——查不到就是查不到，返回零余额，不铸币；
//! - 激活码加了 HMAC 校验位（见 crypto.rs::verify_license），修掉了"格式对就能兑"
//!   那个约 36^12 张免费券的洞。

use serde::{Deserialize, Serialize};

use crate::crypto;
use crate::store::Store;

/// 注册成功时发放的体验额度。
///
/// 数值比改造前的 9999 小得多，因为门槛的性质变了：以前删个文件就能重来，
/// 数值大小无所谓；现在每领一次要占掉一个手机号 + 一条真实计费短信，
/// 但"换个手机号再注册"仍然是可能的，所以额度按"够把功能试明白"给，
/// 不按"够长期白嫖"给。
pub const WELCOME_BALANCE: i64 = 2000;

/// 激活码兑换赠送的灵魂币。
pub const ACTIVATE_BONUS: i64 = 100;

/// 充值档位：(人民币元, 灵魂币)。服务器是权威，客户端那份静态表以后应该改成
/// 从 `/v1/topup/tiers` 拉，现在两边各留一份，改一边记得改另一边。
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

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct Account {
    pub balance: i64,
    pub activated: bool,
    #[serde(default)]
    pub redeemed_keys: Vec<String>,
}

/// 账户在 `accounts` 表里的 key。唯一的构造入口——不要在别处手拼 `"u:"` 前缀，
/// 也不要让任何来自请求体的字符串直接当 key 用。
pub fn account_key(user_id: &str) -> String {
    format!("u:{user_id}")
}

impl Store {
    /// 读账户。**不建账户、不发币**：查不到就返回全零，这是和老 `snapshot()` 最大的区别。
    pub fn account(&self, user_id: &str) -> Account {
        let key = account_key(user_id);
        self.read(|st| st.accounts.get(&key).cloned().unwrap_or_default())
    }

    /// 扣费。余额不会变成负数。
    pub fn consume(&self, user_id: &str, coins: i64) -> Result<Account, String> {
        let key = account_key(user_id);
        self.write(|st| {
            let acc = st.accounts.entry(key).or_default();
            acc.balance = (acc.balance - coins.max(0)).max(0);
            Ok(acc.clone())
        })
    }

    /// 加币。内部用（注册发欢迎币、充值、激活）。
    pub fn add_coins(&self, user_id: &str, coins: i64) -> Result<Account, String> {
        let key = account_key(user_id);
        self.write(|st| {
            let acc = st.accounts.entry(key).or_default();
            acc.balance += coins.max(0);
            Ok(acc.clone())
        })
    }

    /// 充值：只接受预设档位。
    ///
    /// 注意这个接口**现在仍然是"点了就免费到账"**（还没接真实支付网关），
    /// 所以它必须在鉴权之后才可达，而且要限流 + 记审计日志——改造前它是无鉴权的，
    /// 任何人 curl 一句就能给自己发最大档，这次一并堵上（见 main.rs 的 topup handler）。
    pub fn topup(&self, user_id: &str, coins: i64) -> Result<Account, String> {
        if !TOPUP_TIERS.iter().any(|&(_, tier)| tier == coins) {
            return Err("不是预设的充值档位。".to_string());
        }
        self.add_coins(user_id, coins)
    }

    /// 激活码兑换。
    ///
    /// 两道关：
    /// 1. `crypto::verify_license` 验 HMAC 校验位——伪造需要服务器环境变量里的 pepper；
    /// 2. 全表扫一遍确认这个码没被**任何**账号用过——防的是"一个码转发给好几个人
    ///    各自兑换"，这件事本地版架构上做不到，是服务端版本真正的增量价值。
    pub fn activate(&self, user_id: &str, license_key: &str, pepper: &[u8]) -> Result<Account, String> {
        if !crypto::verify_license(pepper, license_key) {
            return Err("激活码无效（示例：SOUL-AB2C-D3EF-GH4J）。".to_string());
        }
        let canonical = crypto::canonical_license(license_key);
        let key = account_key(user_id);
        self.write(|st| {
            let already_used = st
                .accounts
                .values()
                .any(|a| a.redeemed_keys.iter().any(|k| k.eq_ignore_ascii_case(&canonical)));
            if already_used {
                return Err("这个激活码已经被使用过了。".to_string());
            }
            let acc = st.accounts.entry(key).or_default();
            acc.activated = true;
            acc.balance += ACTIVATE_BONUS;
            acc.redeemed_keys.push(canonical);
            Ok(acc.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEPPER: &[u8] = b"test-pepper";

    #[test]
    fn unknown_user_reads_as_zero_and_does_not_mint() {
        // 这是老实现那个「水龙头」的回归测试：改造前对任意没见过的 key
        // 调 snapshot 都会凭空发 9999。
        let store = Store::in_memory();
        let acc = store.account("从没见过的人");
        assert_eq!(acc.balance, 0);
        assert!(!acc.activated);
        assert!(store.read(|st| st.accounts.is_empty()), "只读查询不该在账本里留下条目");
    }

    #[test]
    fn consume_floors_at_zero() {
        let store = Store::in_memory();
        store.add_coins("alice", 5000).unwrap();
        assert_eq!(store.consume("alice", 4000).unwrap().balance, 1000);
        assert_eq!(store.consume("alice", 999_999).unwrap().balance, 0, "余额不该变负数");
    }

    #[test]
    fn activate_adds_bonus_not_overwrites() {
        let store = Store::in_memory();
        store.add_coins("alice", 5000).unwrap();
        let key = crypto::generate_license(PEPPER);
        let acc = store.activate("alice", &key, PEPPER).unwrap();
        assert_eq!(acc.balance, 5000 + ACTIVATE_BONUS, "激活应该叠加而不是覆盖已有余额");
        assert!(acc.activated);
    }

    #[test]
    fn same_license_cannot_be_used_twice_across_accounts() {
        let store = Store::in_memory();
        let key = crypto::generate_license(PEPPER);
        store.activate("alice", &key, PEPPER).unwrap();
        assert!(
            store.activate("bob", &key, PEPPER).is_err(),
            "同一个码不该能被另一个账号再兑一次"
        );
    }

    #[test]
    fn forged_license_is_rejected_without_touching_balance() {
        let store = Store::in_memory();
        store.add_coins("alice", 100).unwrap();
        // 格式完全合法、只是不是我们签出来的
        assert!(store.activate("alice", "SOUL-AAAA-AAAA-AAAB", PEPPER).is_err());
        assert_eq!(store.account("alice").balance, 100);
    }

    #[test]
    fn topup_accepts_only_preset_tiers() {
        let store = Store::in_memory();
        assert!(store.topup("alice", 999).is_err());
        assert_eq!(store.topup("alice", 1000).unwrap().balance, 1000);
    }

    #[test]
    fn account_key_is_namespaced() {
        // u: 前缀是账户命名空间的唯一入口。任何从请求体来的字符串都不该
        // 能直接变成账本 key，否则"拿别人的 user_id 当自己的账户名"这种
        // 越权就成立了。
        assert_eq!(account_key("abc"), "u:abc");
    }
}
