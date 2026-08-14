//! 计费 / 账号验证（骨架）。
//!
//! 变现设想：客户端内置激活码或登录，每次 AI 调用按真实 token 用量折算扣费
//! （单位"灵魂币"，换算逻辑在 ai.rs 的 coins_to_charge）。收费逻辑放在 Rust
//! 后端而不进 webview，避免前端被改。
//!
//! 现状：这仍是**占位骨架**——激活码只做格式校验（随便编一个格式对的就能过），
//! 充值（billing_recharge）是免费直接加余额，没有接任何支付网关，也没有服务端
//! 校验。目的是先把接口形状、充值档位、扣费逻辑定下来，等真要变现时再接
//! 支付网关/发卡/账号服务端，届时只需替换 `billing_recharge`/`billing_activate`
//! 的内部实现，前端调用方式不用变。
//!
//! 余额和激活状态会落盘（见 `state_path`）。这一步是变现的前提：在此之前状态
//! 只活在内存里，重启就回到初始额度，等于无限免费。落盘之后初始额度变成
//! "首次安装送一次"的体验额度，用完就得充值/激活。
//!
//! 落盘之后立刻会有下一个问题：状态文件是纯文本 JSON，用户拿记事本把 balance
//! 后面的数字改大，保存、重启就生效——这台机器是用户自己的，他对自己的文件
//! 有完全的读写权限，任何本地校验都无法从根本上禁止这件事，只能提高门槛。
//! 这里加的是 HMAC 签名（见 `signature`）：写盘时把状态和一个内置密钥一起算出
//! 签名存进文件，读盘时重新算一遍，对不上就判定为被手动改过，不予信任。
//! 这挡得住"打开记事本改一个数字"，挡不住愿意拆这个程序找出密钥的人——密钥
//! 编译进了这个二进制里，对真下决心逆向的人来说本来就是可读的。真正做到
//! "改了也没用"，只能把余额的权威数据挪到用户碰不到的服务器上，这是本模块
//! 顶部注释里"等真要变现时再接账号服务端"要做的事，现在还没有服务器。

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Mutex;

type HmacSha256 = Hmac<Sha256>;

/// 签名用的内置密钥。它跟着程序一起分发到用户机器上，对愿意拆包/反编译的人
/// 来说不是真正意义上的秘密——这一点在任何客户端本地校验里都是天然的，
/// 不是这里实现得不够好。它能挡住的只是"没意愿逆向、单纯拿记事本改数字"
/// 这一档，这也是加这层签名唯一想解决的问题。
const INTEGRITY_KEY: &[u8] = b"soul-lantern-local-billing-integrity-v1-not-a-real-secret";

/// 给状态的关键字段算一个签名。故意手动按固定顺序拼字段，而不是直接对整份
/// JSON 文本取哈希——那样的话 JSON 序列化时字段顺序、空格这些跟数值无关的
/// 细节变了，签名就会跟着变，会把无害的格式变化也误判成篡改。
fn signature(state: &AuthState) -> String {
    let mut mac = HmacSha256::new_from_slice(INTEGRITY_KEY).expect("固定长度的密钥总是合法");
    mac.update(&[state.activated as u8]);
    mac.update(state.license_key.as_deref().unwrap_or("").as_bytes());
    mac.update(&state.balance.to_le_bytes());
    for key in &state.redeemed_keys {
        mac.update(key.as_bytes());
    }
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// 落盘时在 AuthState 基础上多带一个签名字段；`sig` 用 Option 是因为要兼容
/// 加签名校验之前（上一版本）写出去的旧文件——那些文件没有这个字段。
#[derive(Serialize, Deserialize)]
struct SignedState {
    #[serde(flatten)]
    state: AuthState,
    #[serde(default)]
    sig: Option<String>,
}

/// 首次安装赠送的体验额度。按每次生成几十灵魂币算，够用一百多次。
/// 真开始收费时按需调小；注意它只在"配置文件还不存在"时发放一次，
/// 不是每次启动都给（那正是加持久化之前的老问题）。
const WELCOME_BALANCE: i64 = 9999;

#[derive(Clone, Serialize, Deserialize)]
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
        AuthState {
            activated: true,
            license_key: Some("builtin".to_string()),
            balance: WELCOME_BALANCE,
            redeemed_keys: Vec::new(),
        }
    }
}

/// 状态文件位置：<配置目录>/soul-lantern/billing.json。
/// 定位不到配置目录（罕见）就返回 None，此时退化成纯内存状态——
/// 功能照常可用，只是重启不保留，总比直接崩了强。
fn state_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("soul-lantern").join("billing.json"))
}

/// Tauri 托管的全局计费状态。
pub struct Billing {
    pub state: Mutex<AuthState>,
    /// None 表示不落盘（定位不到配置目录，或单元测试里刻意不写文件）。
    path: Option<PathBuf>,
}

impl Default for Billing {
    fn default() -> Self {
        Billing { state: Mutex::new(AuthState::default()), path: None }
    }
}

impl Billing {
    /// 从磁盘读回上次的状态；文件不存在就发放首次体验额度并立刻落盘。
    pub fn load_or_default() -> Self {
        match state_path() {
            Some(path) => Self::load_from(path),
            None => Billing::default(),
        }
    }

    /// `load_or_default` 的实际逻辑，接受可注入的路径方便测试
    /// （真实路径来自系统配置目录，测试不该依赖那个）。
    ///
    /// 三种结果分开处理，别混在一起：
    ///   1. 文件不存在 → 第一次运行，发体验额度并立刻落盘（否则"首次赠送"会在
    ///      下次启动前一直重发）。
    ///   2. 文件存在但连 JSON 都解析不了（写到一半断电、手滑删了一半内容）→
    ///      这大概率是意外损坏而不是故意的，退回默认值但把坏文件原样留在原地
    ///      不覆盖——用户的余额记录比一次干净启动重要，留着还有人工恢复的机会。
    ///   3. 文件是合法 JSON、字段也对，但签名核对不上 → 这不是"意外损坏"，
    ///      是"内容本身被改过但改的人不知道/不管签名"，退回默认值，并且**要**
    ///      覆盖写盘（不像情形 2 那样保留），因为这份数字本来就不该被信任，
    ///      留着它没有情形 2 那种"也许还能抢救"的意义。
    ///      · 签名字段缺失（sig: None）是特例：这是加签名之前的老版本写的正常
    ///        文件，不是篡改，直接信任里面的数字，然后立刻补签名。
    fn load_from(path: PathBuf) -> Self {
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<SignedState>(&text) {
                Ok(signed) => {
                    let expected = signature(&signed.state);
                    let state = match &signed.sig {
                        None => signed.state, // 老版本文件，没有签名，信任并在下面补签
                        Some(sig) if *sig == expected => signed.state,
                        Some(_) => {
                            eprintln!("计费状态签名核对失败，疑似被直接改动，已重置为默认值。");
                            AuthState::default()
                        }
                    };
                    let billing = Billing { state: Mutex::new(state), path: Some(path) };
                    billing.persist(); // 补签名 / 把重置后的默认值写回去
                    billing
                }
                Err(e) => {
                    eprintln!("计费状态文件解析失败（保留原文件不覆盖）：{e}");
                    Billing { state: Mutex::new(AuthState::default()), path: None }
                }
            },
            Err(_) => {
                let billing = Billing { state: Mutex::new(AuthState::default()), path: Some(path) };
                billing.persist();
                billing
            }
        }
    }

    /// 单元测试用：指定状态文件路径，但不读取已有内容（用来测试全新写入场景）。
    /// 要测试"读回已有文件"的路径，用 `load_from`。
    #[cfg(test)]
    fn with_path(path: PathBuf) -> Self {
        Billing { state: Mutex::new(AuthState::default()), path: Some(path) }
    }

    /// 把当前状态连同签名一起写回磁盘。写失败只打日志不打断调用方——
    /// 扣费/充值本身已经在内存里生效了，为了存盘失败去回滚反而更难理解。
    fn persist(&self) {
        let (Some(path), Ok(state)) = (self.path.as_ref(), self.state.lock()) else {
            return;
        };
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("创建计费状态目录失败：{e}");
                return;
            }
        }
        let signed = SignedState { state: state.clone(), sig: Some(signature(&state)) };
        match serde_json::to_string_pretty(&signed) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    eprintln!("写入计费状态失败：{e}");
                }
            }
            Err(e) => eprintln!("序列化计费状态失败：{e}"),
        }
    }

    /// 当前余额快照。
    pub fn balance(&self) -> i64 {
        self.state.lock().map(|st| st.balance).unwrap_or(0)
    }

    /// 扣减指定数量的灵魂币，返回扣减后的余额（最低 0）。
    pub fn consume(&self, coins: i64) -> i64 {
        let after = match self.state.lock() {
            Ok(mut st) => {
                st.balance = (st.balance - coins.max(0)).max(0);
                st.balance
            }
            Err(_) => return 0,
        };
        self.persist();
        after
    }

    /// 增加余额，返回增加后的余额。当前是免费加余额（充值功能还没接真实支付），
    /// 真要收钱时应该把校验/入账逻辑挪到有支付回调可信来源的地方，而不是直接信前端传参。
    pub fn add(&self, coins: i64) -> i64 {
        let after = match self.state.lock() {
            Ok(mut st) => {
                st.balance += coins.max(0);
                st.balance
            }
            Err(_) => return 0,
        };
        self.persist();
        after
    }
}

/// 读取账号 / 余额状态。
#[tauri::command]
pub fn billing_state(billing: tauri::State<'_, Billing>) -> AuthState {
    billing.state.lock().map(|st| st.clone()).unwrap_or_default()
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
    Ok(billing.state.lock().map(|st| st.clone()).unwrap_or_default())
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
    // 花括号限定锁的作用域：persist() 内部还要再拿一次同一把锁，
    // 不先放开会死锁。
    let updated = {
        let mut st = billing.state.lock().map_err(|_| "计费状态锁已损坏".to_string())?;

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
        st.clone()
    };
    billing.persist();
    Ok(updated)
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
            let mut st = billing.state.lock().unwrap();
            st.balance = 100;
        }
        assert_eq!(billing.consume(30), 70);
        assert_eq!(billing.consume(1000), 0, "余额不应变成负数");
    }

    #[test]
    fn add_increments_balance() {
        let billing = Billing::default();
        {
            let mut st = billing.state.lock().unwrap();
            st.balance = 0;
        }
        assert_eq!(billing.add(500), 500);
        assert_eq!(billing.add(300), 800);
    }

    #[test]
    fn recharge_accepts_only_preset_tiers() {
        let billing = Billing::default();
        {
            let mut st = billing.state.lock().unwrap();
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
            let mut st = billing.state.lock().unwrap();
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
            let mut st = billing.state.lock().unwrap();
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
            let mut st = billing.state.lock().unwrap();
            st.balance = 42;
        }
        assert!(activate(&billing, "NOPE").is_err());
        assert_eq!(billing.balance(), 42);
    }

    fn temp_state_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-billing-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("billing.json")
    }

    /// 读回落盘的状态，模拟"重启软件"。
    fn reload(path: &PathBuf) -> AuthState {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn balance_survives_restart() {
        // 加持久化之前最要命的问题：余额只在内存里，重启就回到初始额度，等于无限免费。
        let path = temp_state_file("restart");
        let billing = Billing::with_path(path.clone());
        billing.consume(4000);
        assert_eq!(reload(&path).balance, WELCOME_BALANCE - 4000, "扣费应该落盘");

        billing.add(1000);
        assert_eq!(reload(&path).balance, WELCOME_BALANCE - 4000 + 1000, "充值也应该落盘");
    }

    #[test]
    fn redeemed_keys_survive_restart() {
        // 已兑换的码必须一起落盘，否则重启后同一个码又能再兑一次
        let path = temp_state_file("redeemed");
        let billing = Billing::with_path(path.clone());
        activate(&billing, "SOUL-AB12-CD34-EF56").unwrap();

        let persisted = reload(&path);
        assert_eq!(persisted.redeemed_keys, vec!["SOUL-AB12-CD34-EF56".to_string()]);
        assert_eq!(persisted.balance, WELCOME_BALANCE + ACTIVATE_BONUS);

        // 用落盘的状态重建（模拟重启），同一个码不该还能兑
        let restarted = Billing { state: Mutex::new(persisted), path: Some(path) };
        assert!(activate(&restarted, "SOUL-AB12-CD34-EF56").is_err(), "重启后同一个码仍应被拒");
    }

    #[test]
    fn corrupt_state_file_is_not_overwritten() {
        // 文件坏了宁可退回默认值也不要清空用户记录——留着还有救
        let path = temp_state_file("corrupt");
        std::fs::write(&path, "{ 这不是合法 json").unwrap();
        let parsed = serde_json::from_str::<AuthState>(&std::fs::read_to_string(&path).unwrap());
        assert!(parsed.is_err(), "前置条件：这份文件确实解析不了");

        // load_or_default 遇到解析失败会把 path 置空（不落盘），这里直接验证那个行为：
        let billing = Billing { state: Mutex::new(AuthState::default()), path: None };
        billing.consume(100);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{ 这不是合法 json",
            "坏文件必须原样保留，不能被覆盖",
        );
    }

    #[test]
    fn hand_edited_balance_is_rejected_on_reload() {
        // 这就是用户提的场景：拿文本编辑器直接把 balance 后面的数字改大。
        let path = temp_state_file("tampered");
        let billing = Billing::with_path(path.clone());
        billing.consume(1000); // WELCOME_BALANCE - 1000，正常落盘，带合法签名

        // 手改文件里的余额，但不知道/不管签名怎么算——这正是"记事本改数字"的行为，
        // 不是能算出合法签名的攻击者。
        let text = std::fs::read_to_string(&path).unwrap();
        let tampered = text.replacen(
            &format!("\"balance\": {}", WELCOME_BALANCE - 1000),
            "\"balance\": 999999999",
            1,
        );
        assert_ne!(text, tampered, "前置条件：确实替换到了 balance 字段");
        std::fs::write(&path, &tampered).unwrap();

        // 重新"启动"：签名对不上，不该采信这个改过的天文数字，应该重置为默认值
        let reloaded = Billing::load_from(path.clone());
        assert_eq!(reloaded.balance(), WELCOME_BALANCE, "签名不匹配就不该信任文件里的余额");

        // 而且这次要覆盖写盘（不同于纯损坏 JSON 的情形）：被篡改的数字不值得保留，
        // 磁盘上应该已经是一份新的、签名合法的默认状态。
        assert_eq!(reload(&path).balance, WELCOME_BALANCE);
    }

    #[test]
    fn legacy_file_without_signature_is_trusted_then_signed() {
        // 上一版本（加签名校验之前）写出去的文件没有 sig 字段。这批文件不能被当成
        // "被篡改"处理，否则这次更新一上线，所有老用户的余额都会被误判清零。
        let path = temp_state_file("legacy");
        std::fs::write(
            &path,
            r#"{"activated":true,"licenseKey":"builtin","balance":6666,"redeemedKeys":[]}"#,
        )
        .unwrap();

        let billing = Billing::load_from(path.clone());
        assert_eq!(billing.balance(), 6666, "没有签名字段的老文件应该被信任，不能清零");

        // 信任之后要立刻补签名，不然下次重启这份"合法"数据又会被当成异常
        let persisted_text = std::fs::read_to_string(&path).unwrap();
        assert!(persisted_text.contains("\"sig\""), "读到老文件后应该马上补签名落盘");

        // 补签名之后，"篡改检测"对这份文件才算真正生效
        let again = Billing::load_from(path.clone());
        assert_eq!(again.balance(), 6666, "补签名后重新读取，余额应保持不变");
    }

    #[test]
    fn no_path_means_memory_only_and_does_not_panic() {
        // 定位不到配置目录时退化成纯内存，功能照常，不该崩
        let billing = Billing::default();
        assert_eq!(billing.consume(1), WELCOME_BALANCE - 1);
        assert_eq!(billing.add(5), WELCOME_BALANCE - 1 + 5);
    }

    #[test]
    fn topup_tiers_match_agreed_table() {
        let tiers = billing_topup_tiers();
        assert_eq!(tiers.len(), 8);
        assert_eq!((tiers[0].yuan, tiers[0].coins), (1.0, 1000));
        assert_eq!((tiers[7].yuan, tiers[7].coins), (648.0, 888888));
    }
}
