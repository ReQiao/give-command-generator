//! 账号体系：注册（手机号验证码）/ 登录 / 找回密码 / 会话。
//!
//! ## 身份是什么
//!
//! 用户名 + 密码 + 手机号。手机号是找回密码的唯一凭据，也是注册时的验证对象；
//! 登录可以用用户名或手机号。改造前的匿名 device_id 已经整个删掉了——它既不能
//! 跨设备找回，又是个"删掉本地文件就重发欢迎币"的水龙头。
//!
//! ## 几个不是洁癖、是真会被打的设计
//!
//! - **注册时不立刻建号**：`register/begin` 只写一条 pending，用户名/手机号的唯一性
//!   判定推迟到 `register/verify` 成功那一刻。否则有人可以用别人的手机号批量 begin，
//!   把好用户名全占死。
//! - **不告诉调用方"这个用户名/手机号已存在"**：本产品没有用户主页、没有排行榜，
//!   用户名除了登录标识没有第二个用途，返回 409 等于凭空造一个枚举器。所以
//!   `register/begin` 恒定返回同一形状，冲突留到 verify 那一刻才说。
//! - **无论手机号是否已注册都先跑一次密码哈希**：不然"已注册"分支 5ms、"未注册"
//!   分支 300ms，`curl -w '%{time_total}'` 一测就把上面那条防护全抵消了。
//! - **登录失败锁定的文案对不存在的账号也要一模一样**：「失败次数过多」这句话本身
//!   就泄露账号存在性。
//! - **限流必须排在密码哈希前面**：哈希是 600k 次 PBKDF2，约 300ms 纯 CPU。
//!   登录接口无鉴权，如果先哈希后限流，一个 200 字节的请求就能换 300ms CPU，
//!   这是放大型 DoS。
//! - **哈希信号量用 try_acquire 而不是 acquire**：`Semaphore::acquire()` 是无界排队的，
//!   拿不到就一直等，攻击者能把队列堆到内存耗尽。拿不到直接告诉用户"服务器忙"。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::crypto;
use crate::ledger;
use crate::sms::{self, Purpose};
use crate::store::{now_secs, Store};
use crate::{ApiError, AppState};
use std::sync::Arc;

/// 会话有效期。到期后客户端会拿到 401，自动清掉本地 token 回到登录界面。
const SESSION_TTL_SECS: u64 = 30 * 24 * 3600;
/// pending 记录的存活时间。比短信验证码本身的有效期长一点，
/// 好让"码过期了"能给出比"请求不存在"更准确的提示。
const PENDING_TTL_SECS: u64 = 15 * 60;
/// 一条 pending 最多能验错几次。太小会变成定点骚扰工具（攻击者故意打错把受害者
/// 刚收到的码作废），太大就成了爆破 6 位码的窗口。
const MAX_VERIFY_ATTEMPTS: u32 = 8;

// ---------------------------------------------------------------- 数据模型

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    /// 判重用的规范形式（全角转半角 + 小写）。不要拿 `username` 直接比。
    pub username_key: String,
    pub phone: String,
    pub password_hash: Vec<u8>,
    pub password_salt: Vec<u8>,
    /// 存下来而不是读常量：以后调大迭代次数，老用户可以在下次登录成功时顺手升级，
    /// 不用强制所有人改密码。
    pub pbkdf2_iterations: u32,
    pub created_at: u64,
    pub last_login_at: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Session {
    pub user_id: String,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PendingVerification {
    pub kind: String,
    pub phone: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub attempts: u32,
    // 以下只有注册流程才有：verify 成功那一刻才拿这些字段真正建号
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub username_key: Option<String>,
    #[serde(default)]
    pub password_hash: Option<Vec<u8>>,
    #[serde(default)]
    pub password_salt: Option<Vec<u8>>,
    #[serde(default)]
    pub pbkdf2_iterations: Option<u32>,
    /// 只有 SMS_KIND=log 时才有值：Log 后端的验证码是我们自己生成的，
    /// 得自己存自己比对（阿里云那条路上码由阿里云托管，这里是 None）。
    #[serde(default)]
    pub local_code_hmac: Option<Vec<u8>>,
}

fn pending_key(purpose: Purpose, phone: &str) -> String {
    format!("{}:{phone}", purpose.key_prefix())
}

// ---------------------------------------------------------------- 用户名 / 密码 校验

/// 判重用的规范化。
///
/// 没有引入 `unicode-normalization` 做完整 NFKC——那是个新依赖，而这里真正要防的
/// 只有一件事：全角字符冒充半角（`ａｄｍｉｎ` vs `admin`）。全角 ASCII 在
/// U+FF01..=U+FF5E，减 0xFEE0 就是对应半角，十行搞定。
pub fn normalize_username_key(name: &str) -> String {
    name.chars()
        .map(|c| {
            let cp = c as u32;
            if (0xFF01..=0xFF5E).contains(&cp) {
                char::from_u32(cp - 0xFEE0).unwrap_or(c)
            } else if cp == 0x3000 {
                ' ' // 表意空格，规范化之后会被下面的空白检查拒掉
            } else {
                c
            }
        })
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 用户名允许中文——目标人群是中国 MC 玩家，强制 ASCII 是需求的隐性收窄。
pub fn validate_username(name: &str) -> Result<(), String> {
    let n = name.chars().count();
    if !(2..=24).contains(&n) {
        return Err("用户名需要 2~24 个字符。".to_string());
    }
    for c in name.chars() {
        if c.is_whitespace() {
            return Err("用户名里不能有空格。".to_string());
        }
        if c.is_control() {
            return Err("用户名里有不可见的控制字符，请重新输入。".to_string());
        }
        // 零宽字符 / 双向控制符：肉眼看不见，但能拿来伪造成别人的用户名
        let cp = c as u32;
        if matches!(cp, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0xFEFF) {
            return Err("用户名里有不可见字符，请重新输入。".to_string());
        }
    }
    Ok(())
}

/// 常见弱口令。
///
/// 这一条对"泄库之后能不能被爆破"的影响，比把 PBKDF2 迭代次数从 60 万提到 100 万
/// 大几个数量级——弱口令字典只有几万条，而迭代次数只是把每次尝试的成本线性放大。
///
/// 列表按中国用户的实际习惯选（拼音串、生日式数字、键盘序、"我爱你"系列），
/// 不是照搬英文世界的 rockyou 前 N 名。以后要扩充，直接往这个数组里加就行——
/// 查询走下面的 `HashSet`，加到几万条也还是 O(1)。
const WEAK_PASSWORDS: &[&str] = &[
    "12345678", "123456789", "1234567890", "123123123", "111111111", "000000000",
    "88888888", "66666666", "11111111", "00000000", "12341234", "12121212",
    "abcd1234", "abc12345", "a1234567", "1qaz2wsx", "qwertyui", "qwerty123",
    "asdfghjk", "zxcvbnm123", "1q2w3e4r", "1qazxsw2", "q1w2e3r4", "1a2b3c4d",
    "password", "password1", "password123", "passw0rd", "p@ssw0rd", "admin123",
    "administrator", "root1234", "toor1234", "letmein1", "welcome1", "iloveyou",
    "sunshine1", "princess1", "football1", "baseball1", "dragon123", "monkey123",
    "woaini1314", "woaini520", "woaini123", "5201314520", "1314520520", "520520520",
    "wangyifan", "zhangwei123", "liwei1234", "wangfang123", "aizhenzhu",
    "woshishui", "nihaoma123", "zhongguo123", "beijing123", "shanghai123",
    "chenxiaoming", "xiaoming123", "xiaohong123", "zhangsan123", "lisi1234",
    "qq123456", "qq1234567", "wechat123", "weixin123", "taobao123", "alipay123",
    "minecraft", "minecraft1", "minecraft123", "mojang123", "notch123",
    "steve1234", "herobrine", "creeper123", "diamond123", "redstone1",
    "lingdeng123", "soullantern", "hunling123", "denglong123",
    "asdasdasd", "qweqweqwe", "zxczxczxc", "aaaaaaaa", "aaaa1111", "abcabcabc",
    "19900101", "19951231", "20001231", "20080808", "19891989", "20202020",
];

fn weak_password_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| WEAK_PASSWORDS.iter().copied().collect())
}

pub fn validate_password(password: &str, username: &str, phone: &str) -> Result<(), String> {
    let n = password.chars().count();
    if n < 8 {
        return Err("密码至少 8 个字符。".to_string());
    }
    if n > 128 {
        return Err("密码太长了（最多 128 个字符）。".to_string());
    }
    let lower = password.to_lowercase();
    if weak_password_set().contains(lower.as_str()) {
        return Err("这个密码太常见了，换一个不容易被猜到的。".to_string());
    }
    // 全是同一个字符
    let mut chars = password.chars();
    if let Some(first) = chars.next() {
        if chars.all(|c| c == first) {
            return Err("密码不能是同一个字符重复。".to_string());
        }
    }
    if !username.is_empty() && lower == username.to_lowercase() {
        return Err("密码不能和用户名一样。".to_string());
    }
    if !phone.is_empty() && (lower.contains(phone) || phone.contains(&lower)) {
        return Err("密码不能包含手机号。".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------- 密码哈希（有界并发）

fn hash_semaphore() -> &'static tokio::sync::Semaphore {
    static SEM: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    SEM.get_or_init(|| {
        // 按核数给，但夹在 [2, 8]：太小则正常用户互相排队，太大则一波并发就把 CPU 吃光。
        let n = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(2).clamp(2, 8);
        tokio::sync::Semaphore::new(n)
    })
}

/// 跑一次 PBKDF2。
///
/// 两件事都不能省：
/// - `spawn_blocking`：600k 次 PBKDF2 是 100~400ms 纯 CPU，直接在 async handler 里跑
///   会占死 tokio 的 worker 线程，把 AI 代理那条线一起卡住。
/// - `try_acquire`：见模块头注释，无界排队会被打爆内存。
async fn hash_password(password: String, salt: Vec<u8>, iterations: u32) -> Result<Vec<u8>, String> {
    let _permit = hash_semaphore()
        .try_acquire()
        .map_err(|_| "服务器忙，请稍后再试。".to_string())?;
    tokio::task::spawn_blocking(move || crypto::pbkdf2_hash(&password, &salt, iterations))
        .await
        .map_err(|e| {
            tracing::error!("密码哈希任务失败：{e}");
            "服务器内部错误。".to_string()
        })
}

async fn verify_password(
    password: String,
    salt: Vec<u8>,
    iterations: u32,
    expected: Vec<u8>,
) -> Result<bool, String> {
    let _permit = hash_semaphore()
        .try_acquire()
        .map_err(|_| "服务器忙，请稍后再试。".to_string())?;
    tokio::task::spawn_blocking(move || {
        crypto::pbkdf2_verify(&password, &salt, iterations, &expected)
    })
    .await
    .map_err(|e| {
        tracing::error!("密码校验任务失败：{e}");
        "服务器内部错误。".to_string()
    })
}

/// 给"用户不存在"分支用的假哈希，抹平时间差。参数必须和真实分支完全一致，
/// 否则时间差还在，只是变小了。
async fn dummy_hash_work() {
    static DUMMY_SALT: OnceLock<Vec<u8>> = OnceLock::new();
    let salt = DUMMY_SALT.get_or_init(|| crypto::random_bytes(crypto::SALT_LEN)).clone();
    let _ = hash_password("dummy-password-for-timing".to_string(), salt, crypto::PBKDF2_ITERATIONS)
        .await;
}

// ---------------------------------------------------------------- 限流

/// 滑动窗口计数器，纯内存。
///
/// 刻意不落盘：限流状态每次请求都要写，落盘就是每次请求一次 fsync。重启后计数清零
/// 是可以接受的——攻击者没法主动让服务重启，而服务重启本身就很罕见。
#[derive(Default)]
pub struct RateLimiter {
    hits: Mutex<HashMap<String, Vec<u64>>>,
}

impl RateLimiter {
    /// 检查并记录一次。窗口内超过 `limit` 就返回 Err(还需等待的秒数)。
    pub fn check(&self, key: &str, limit: usize, window_secs: u64) -> Result<(), u64> {
        let now = now_secs();
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.saturating_sub(*t) < window_secs);
        if entry.len() >= limit {
            let oldest = entry.first().copied().unwrap_or(now);
            return Err(window_secs.saturating_sub(now.saturating_sub(oldest)).max(1));
        }
        entry.push(now);
        Ok(())
    }

    /// 只看不记（用于"先探一下会不会超"的场景）。
    pub fn peek(&self, key: &str, limit: usize, window_secs: u64) -> bool {
        let now = now_secs();
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.saturating_sub(*t) < window_secs);
        entry.len() < limit
    }

    /// 撤销最近一次记录（用于"限流已记但后续校验失败、不该占用配额"的回滚）。
    pub fn refund(&self, key: &str) {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = hits.get_mut(key) {
            entry.pop();
        }
    }

    /// 定期清理：没有这一步，`hits` 会随着攻击者变换 key 无限增长。
    pub fn sweep(&self, max_window_secs: u64) {
        let now = now_secs();
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        hits.retain(|_, v| {
            v.retain(|t| now.saturating_sub(*t) < max_window_secs);
            !v.is_empty()
        });
    }

    pub fn count(&self, key: &str, window_secs: u64) -> usize {
        let now = now_secs();
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.saturating_sub(*t) < window_secs);
        entry.len()
    }
}

const DAY: u64 = 24 * 3600;
const HOUR: u64 = 3600;

/// 每日全局发信总量上限。
///
/// **分层预算**是关键：如果全局配额先到先得，攻击者用几个 IP 就能把当天额度打光，
/// 于是当天所有真实用户既注册不了也找不回密码。所以给"重发"和"密码重置"
/// 单独留一份配额——这两类的目标手机号一定是系统已知的，攻击者要占用得先有账号。
fn daily_sms_cap() -> usize {
    std::env::var("SMS_DAILY_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(200)
}
/// 留给"已知手机号"（重发 / 密码重置）的比例。
const KNOWN_PHONE_RESERVE: f64 = 0.4;

/// 同一手机号两次发码之间的最小间隔。
///
/// 做成可配置有两个理由：一是运维上确实可能想调（比如短信到达变慢时放宽一点），
/// 二是集成测试需要在一个用例里连续走注册和找回密码两条流程，设成 0 才跑得动。
/// **注意这个冷却是 register / resend / reset 三条流程共用的**——分开计数
/// 等于直接送攻击者三倍的短信配额。
fn sms_min_interval_secs() -> u64 {
    std::env::var("SMS_MIN_INTERVAL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60)
}

fn client_ip(addr: &SocketAddr) -> String {
    addr.ip().to_string()
}

/// 发短信前的完整限流检查。`is_known_phone` 决定用哪一档全局预算。
fn check_sms_quota(rl: &RateLimiter, phone: &str, ip: &str, is_known_phone: bool) -> Result<(), String> {
    // 同一手机号：冷却期内 1 条、24 小时 5 条
    let interval = sms_min_interval_secs();
    if interval > 0 {
        if let Err(wait) = rl.check(&format!("sms:phone:{phone}"), 1, interval) {
            return Err(format!("验证码刚发过，请 {wait} 秒后再试。"));
        }
    }
    if rl.check(&format!("sms:phone24:{phone}"), 5, DAY).is_err() {
        rl.refund(&format!("sms:phone:{phone}"));
        return Err("这个手机号今天的验证码次数用完了，请明天再试。".to_string());
    }
    // 同一 IP：1 小时 5 条、24 小时 10 条。正常人一天注册 5 次已经极其反常。
    if rl.check(&format!("sms:ip:{ip}"), 5, HOUR).is_err() {
        rl.refund(&format!("sms:phone:{phone}"));
        rl.refund(&format!("sms:phone24:{phone}"));
        return Err("操作太频繁了，请稍后再试。".to_string());
    }
    if rl.check(&format!("sms:ip24:{ip}"), 10, DAY).is_err() {
        rl.refund(&format!("sms:phone:{phone}"));
        rl.refund(&format!("sms:phone24:{phone}"));
        rl.refund(&format!("sms:ip:{ip}"));
        return Err("操作太频繁了，请明天再试。".to_string());
    }

    let cap = daily_sms_cap();
    let new_phone_budget = ((cap as f64) * (1.0 - KNOWN_PHONE_RESERVE)) as usize;
    let budget = if is_known_phone { cap } else { new_phone_budget };
    if rl.check("sms:global", budget, DAY).is_err() {
        rl.refund(&format!("sms:phone:{phone}"));
        rl.refund(&format!("sms:phone24:{phone}"));
        rl.refund(&format!("sms:ip:{ip}"));
        rl.refund(&format!("sms:ip24:{ip}"));
        // 用 error! 单独记一行：否则某天所有人都注册不了，你会先去查阿里云、查余额、
        // 查网络，最后才想到是被人把配额刷满了。
        tracing::error!(
            "每日短信配额已用尽（cap={cap}，新号段预算={new_phone_budget}）。\
             要么是真实用量涨了，要么是被刷了——去 /v1/admin/stats 看 smsSentToday。"
        );
        return Err("今天的验证码发送量已达上限，请明天再试。".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------- 会话

fn session_key(pepper: &[u8], token: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(crypto::hmac_sha256(pepper, token.as_bytes()))
}

/// 从 Authorization 头里取 Bearer token。
pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 校验 Bearer token，返回 user_id。所有需要登录的端点都从这里进。
pub fn require_user(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = bearer_token(headers)
        .ok_or((StatusCode::UNAUTHORIZED, "请先登录。".to_string()))?;
    let key = session_key(&state.auth_pepper, &token);
    let now = now_secs();
    let user_id = state.store.read(|st| {
        st.sessions.get(&key).and_then(|s| (s.expires_at > now).then(|| s.user_id.clone()))
    });
    user_id.ok_or((StatusCode::UNAUTHORIZED, "登录已过期，请重新登录。".to_string()))
}

fn issue_session(state: &AppState, user_id: &str) -> Result<(String, u64), String> {
    let token = crypto::random_token();
    let key = session_key(&state.auth_pepper, &token);
    let now = now_secs();
    let expires_at = now + SESSION_TTL_SECS;
    state.store.write(|st| {
        // 顺手清理过期会话，省得表无限长
        st.sessions.retain(|_, s| s.expires_at > now);
        st.sessions.insert(key, Session { user_id: user_id.to_string(), created_at: now, expires_at });
        Ok(())
    })?;
    Ok((token, expires_at))
}

fn revoke_all_sessions(state: &AppState, user_id: &str) -> Result<(), String> {
    state.store.write(|st| {
        st.sessions.retain(|_, s| s.user_id != user_id);
        Ok(())
    })
}

// ---------------------------------------------------------------- 对外视图

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub username: String,
    pub phone_masked: String,
    pub created_at: u64,
}

impl From<&User> for UserView {
    fn from(u: &User) -> Self {
        UserView {
            username: u.username.clone(),
            phone_masked: crypto::mask_phone(&u.phone),
            created_at: u.created_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub ok: bool,
    pub token: String,
    pub expires_at: u64,
    pub user: UserView,
    pub balance: i64,
    pub activated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSentView {
    pub ok: bool,
    pub phone_masked: String,
    pub expires_in_secs: u32,
    /// SMS_KIND=log 时为 true，客户端据此在界面上提示"当前是日志模式"。
    pub log_mode: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OkView {
    pub ok: bool,
}

// ---------------------------------------------------------------- 请求体
//
// 注意：请求体一律 snake_case 且**不加** `#[serde(rename_all)]`，响应体一律
// camelCase 且加。这是 src-tauri/src/remote.rs:131 那条血泪注释定下的约定
// （两边字段名对不上时，各自的单测都是绿的，只有真实 HTTP 往返测试能抓到）。

#[derive(Deserialize)]
pub struct RegisterBeginReq {
    username: String,
    password: String,
    phone: String,
}

#[derive(Deserialize)]
pub struct PhoneOnlyReq {
    phone: String,
}

#[derive(Deserialize)]
pub struct RegisterVerifyReq {
    phone: String,
    code: String,
}

#[derive(Deserialize)]
pub struct LoginReq {
    /// 用户名或手机号
    account: String,
    password: String,
}

#[derive(Deserialize)]
pub struct ResetConfirmReq {
    phone: String,
    code: String,
    new_password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordReq {
    old_password: String,
    new_password: String,
}

// ---------------------------------------------------------------- 查找

fn find_user_by_phone(store: &Store, phone: &str) -> Option<User> {
    store.read(|st| st.users.values().find(|u| u.phone == phone).cloned())
}

fn find_user_by_account(store: &Store, account: &str) -> Option<User> {
    let key = normalize_username_key(account);
    let phone = crypto::normalize_phone(account);
    store.read(|st| {
        st.users
            .values()
            .find(|u| u.username_key == key || Some(&u.phone) == phone.as_ref())
            .cloned()
    })
}

// ---------------------------------------------------------------- Handlers

/// 注册第一步：校验输入 → 发验证码 → 写一条 pending。
///
/// 这里**不建号、不占用户名、不占手机号**，唯一性判定推迟到 verify。
async fn register_begin(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<RegisterBeginReq>,
) -> Result<Json<CodeSentView>, ApiError> {
    let ip = client_ip(&addr);
    let username = req.username.trim().to_string();
    let Some(phone) = crypto::normalize_phone(&req.phone) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };

    validate_username(&username).map_err(bad_request)?;
    validate_password(&req.password, &username, &phone).map_err(bad_request)?;

    let known = find_user_by_phone(&state.store, &phone).is_some();

    // 限流在哈希之前——见模块头注释。
    check_sms_quota(&state.rate_limiter, &phone, &ip, known).map_err(too_many)?;

    // 无论手机号是否已注册都跑同一份哈希：抹平 300ms 的时间差，
    // 否则下面"恒定返回同一形状"这个防枚举设计等于白做。
    let salt = crypto::random_bytes(crypto::SALT_LEN);
    let hash = hash_password(req.password.clone(), salt.clone(), crypto::PBKDF2_ITERATIONS)
        .await
        .map_err(service_busy)?;

    if known {
        // 已注册：不建 pending（它在 verify 时注定失败，建了没用），也不发注册验证码，
        // 但**返回和成功完全一样的响应**。
        tracing::info!("注册请求命中已注册手机号 {}（静默忽略）", crypto::mask_phone(&phone));
        return Ok(Json(CodeSentView {
            ok: true,
            phone_masked: crypto::mask_phone(&phone),
            expires_in_secs: sms::CODE_VALID_SECS,
            log_mode: state.sms.is_log(),
        }));
    }

    let local_code = state.sms.send(&phone, Purpose::Register).await.map_err(bad_request)?;
    let local_code_hmac = local_code.map(|c| crypto::hmac_sha256(&state.auth_pepper, c.as_bytes()));

    let now = now_secs();
    state
        .store
        .write(|st| {
            st.pending.retain(|_, p| p.expires_at > now);
            st.pending.insert(
                pending_key(Purpose::Register, &phone),
                PendingVerification {
                    kind: Purpose::Register.key_prefix().to_string(),
                    phone: phone.clone(),
                    created_at: now,
                    expires_at: now + PENDING_TTL_SECS,
                    attempts: 0,
                    username: Some(username),
                    username_key: Some(normalize_username_key(&req.username.trim().to_string())),
                    password_hash: Some(hash),
                    password_salt: Some(salt),
                    pbkdf2_iterations: Some(crypto::PBKDF2_ITERATIONS),
                    local_code_hmac,
                },
            );
            Ok(())
        })
        .map_err(internal)?;

    Ok(Json(CodeSentView {
        ok: true,
        phone_masked: crypto::mask_phone(&phone),
        expires_in_secs: sms::CODE_VALID_SECS,
        log_mode: state.sms.is_log(),
    }))
}

/// 重发注册验证码。和 register/begin 共用同一组限流计数器——
/// 分开计数等于直接送攻击者双倍配额。
async fn register_resend(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<PhoneOnlyReq>,
) -> Result<Json<CodeSentView>, ApiError> {
    let ip = client_ip(&addr);
    let Some(phone) = crypto::normalize_phone(&req.phone) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };
    let key = pending_key(Purpose::Register, &phone);
    let exists = state.store.read(|st| st.pending.contains_key(&key));

    // 已有 pending 的重发算"已知手机号"，走预留配额那一档
    check_sms_quota(&state.rate_limiter, &phone, &ip, exists).map_err(too_many)?;

    if exists {
        let local_code = state.sms.send(&phone, Purpose::Register).await.map_err(bad_request)?;
        let hmac = local_code.map(|c| crypto::hmac_sha256(&state.auth_pepper, c.as_bytes()));
        state
            .store
            .write(|st| {
                if let Some(p) = st.pending.get_mut(&key) {
                    p.expires_at = now_secs() + PENDING_TTL_SECS;
                    p.attempts = 0;
                    p.local_code_hmac = hmac;
                }
                Ok(())
            })
            .map_err(internal)?;
    }

    Ok(Json(CodeSentView {
        ok: true,
        phone_masked: crypto::mask_phone(&phone),
        expires_in_secs: sms::CODE_VALID_SECS,
        log_mode: state.sms.is_log(),
    }))
}

/// 核验一条 pending 的验证码。Log 后端自己比对，阿里云走 CheckSmsVerifyCode。
async fn consume_code(
    state: &AppState,
    purpose: Purpose,
    phone: &str,
    code: &str,
) -> Result<PendingVerification, ApiError> {
    let key = pending_key(purpose, phone);
    let now = now_secs();

    let pending = state
        .store
        .read(|st| st.pending.get(&key).cloned())
        .filter(|p| p.expires_at > now)
        .ok_or((StatusCode::BAD_REQUEST, "验证码已过期，请重新获取。".to_string()))?;

    if pending.attempts >= MAX_VERIFY_ATTEMPTS {
        return Err((StatusCode::TOO_MANY_REQUESTS, "验证码错误次数过多，请重新获取。".to_string()));
    }

    let normalized: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    let ok = match &pending.local_code_hmac {
        Some(expected) => {
            let given = crypto::hmac_sha256(&state.auth_pepper, normalized.as_bytes());
            crypto::constant_time_eq(&given, expected)
        }
        None => state.sms.check(phone, &normalized).await.map_err(bad_request)?,
    };

    if !ok {
        // 记一次失败尝试。写失败不该把用户挡在门外，所以只记日志。
        if let Err(e) = state.store.write(|st| {
            if let Some(p) = st.pending.get_mut(&key) {
                p.attempts += 1;
            }
            Ok(())
        }) {
            tracing::warn!("记录验证码失败次数时落盘失败：{e}");
        }
        return Err((StatusCode::BAD_REQUEST, "验证码不对，请检查后重新输入。".to_string()));
    }

    // 用掉即销毁，防重放
    state
        .store
        .write(|st| {
            st.pending.remove(&key);
            Ok(())
        })
        .map_err(internal)?;
    Ok(pending)
}

/// 注册第二步：验证码对了才真正建号。唯一性冲突在这一刻才判。
async fn register_verify(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterVerifyReq>,
) -> Result<Json<SessionView>, ApiError> {
    let Some(phone) = crypto::normalize_phone(&req.phone) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };
    let pending = consume_code(&state, Purpose::Register, &phone, &req.code).await?;

    let (Some(username), Some(username_key), Some(hash), Some(salt), Some(iters)) = (
        pending.username.clone(),
        pending.username_key.clone(),
        pending.password_hash.clone(),
        pending.password_salt.clone(),
        pending.pbkdf2_iterations,
    ) else {
        return Err((StatusCode::BAD_REQUEST, "注册信息已失效，请重新注册。".to_string()));
    };

    let user_id = crypto::random_token();
    let now = now_secs();

    state
        .store
        .write(|st| {
            if st.users.values().any(|u| u.username_key == username_key) {
                return Err("这个用户名刚刚被别人注册了，换一个吧。".to_string());
            }
            if st.users.values().any(|u| u.phone == phone) {
                return Err("这个手机号已经注册过了，直接登录或找回密码吧。".to_string());
            }
            st.users.insert(
                user_id.clone(),
                User {
                    id: user_id.clone(),
                    username,
                    username_key,
                    phone: phone.clone(),
                    password_hash: hash,
                    password_salt: salt,
                    pbkdf2_iterations: iters,
                    created_at: now,
                    last_login_at: now,
                },
            );
            // 欢迎余额只在这里发一次，全局唯一的铸币点。
            st.accounts.insert(
                ledger::account_key(&user_id),
                ledger::Account {
                    balance: ledger::WELCOME_BALANCE,
                    activated: false,
                    redeemed_keys: Vec::new(),
                },
            );
            Ok(())
        })
        .map_err(bad_request)?;

    tracing::info!("新用户注册成功 user_id={user_id} phone={}", crypto::mask_phone(&phone));
    build_session_response(&state, &user_id).map_err(internal)
}

async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginReq>,
) -> Result<Json<SessionView>, ApiError> {
    let ip = client_ip(&addr);
    let account_key = normalize_username_key(req.account.trim());

    // 限流在哈希之前。两个维度都要有：只按账号限，攻击者换账号就绕过；
    // 只按 IP 限，攻击者换代理就绕过。
    let acct_limit_key = format!("login:acct:{account_key}");
    let ip_limit_key = format!("login:ip:{ip}");
    let locked = state.rate_limiter.check(&acct_limit_key, 5, 15 * 60).is_err()
        || state.rate_limiter.check(&ip_limit_key, 30, 15 * 60).is_err();
    if locked {
        // 注意：这句文案对"账号不存在"也要一模一样地给出去，
        // 否则「失败次数过多」本身就成了账号存在性的探针。
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "登录失败次数过多，请 15 分钟后再试。".to_string(),
        ));
    }

    let user = find_user_by_account(&state.store, req.account.trim());
    let Some(user) = user else {
        // 账号不存在也要跑一次同参数哈希，抹平时间差
        dummy_hash_work().await;
        return Err((StatusCode::UNAUTHORIZED, "用户名或密码不对。".to_string()));
    };

    let ok = verify_password(
        req.password.clone(),
        user.password_salt.clone(),
        user.pbkdf2_iterations,
        user.password_hash.clone(),
    )
    .await
    .map_err(service_busy)?;

    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "用户名或密码不对。".to_string()));
    }

    // 登录成功，把这次失败计数还回去，免得正常用户打错一次密码之后被自己的重试锁住
    state.rate_limiter.refund(&acct_limit_key);
    state.rate_limiter.refund(&ip_limit_key);

    let uid = user.id.clone();
    state
        .store
        .write(|st| {
            if let Some(u) = st.users.get_mut(&uid) {
                u.last_login_at = now_secs();
            }
            Ok(())
        })
        .map_err(internal)?;

    build_session_response(&state, &user.id).map_err(internal)
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<OkView>, ApiError> {
    if let Some(token) = bearer_token(&headers) {
        let key = session_key(&state.auth_pepper, &token);
        state
            .store
            .write(|st| {
                st.sessions.remove(&key);
                Ok(())
            })
            .map_err(internal)?;
    }
    Ok(Json(OkView { ok: true }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeView {
    pub ok: bool,
    pub user: UserView,
    pub balance: i64,
    pub activated: bool,
}

async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Json<MeView>, ApiError> {
    let user_id = require_user(&state, &headers)?;
    let user = state
        .store
        .read(|st| st.users.get(&user_id).cloned())
        .ok_or((StatusCode::UNAUTHORIZED, "登录已过期，请重新登录。".to_string()))?;
    let acc = state.store.account(&user_id);
    Ok(Json(MeView {
        ok: true,
        user: UserView::from(&user),
        balance: acc.balance,
        activated: acc.activated,
    }))
}

/// 找回密码第一步。
///
/// 无论手机号是否注册过都返回同样的东西、耗时也要一样——这里用固定截止时间而不是
/// "随机 sleep 一下"：均匀随机噪声掩盖不了均值差，采样二十次求平均就能分辨。
async fn reset_begin(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<PhoneOnlyReq>,
) -> Result<Json<CodeSentView>, ApiError> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(400);
    let ip = client_ip(&addr);
    let Some(phone) = crypto::normalize_phone(&req.phone) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };

    let exists = find_user_by_phone(&state.store, &phone).is_some();
    // 密码重置永远算"已知手机号"，走预留配额——这是分层预算保护的重点对象：
    // 就算新号注册被刷满，已注册用户也必须还能找回密码。
    let quota = check_sms_quota(&state.rate_limiter, &phone, &ip, true);

    let result = async {
        quota.map_err(too_many)?;
        if exists {
            let local_code = state.sms.send(&phone, Purpose::Reset).await.map_err(bad_request)?;
            let hmac = local_code.map(|c| crypto::hmac_sha256(&state.auth_pepper, c.as_bytes()));
            let now = now_secs();
            state
                .store
                .write(|st| {
                    st.pending.retain(|_, p| p.expires_at > now);
                    st.pending.insert(
                        pending_key(Purpose::Reset, &phone),
                        PendingVerification {
                            kind: Purpose::Reset.key_prefix().to_string(),
                            phone: phone.clone(),
                            created_at: now,
                            expires_at: now + PENDING_TTL_SECS,
                            attempts: 0,
                            username: None,
                            username_key: None,
                            password_hash: None,
                            password_salt: None,
                            pbkdf2_iterations: None,
                            local_code_hmac: hmac,
                        },
                    );
                    Ok(())
                })
                .map_err(internal)?;
        } else {
            tracing::info!("重置密码请求命中未注册手机号 {}（静默忽略）", crypto::mask_phone(&phone));
        }
        Ok(Json(CodeSentView {
            ok: true,
            phone_masked: crypto::mask_phone(&phone),
            expires_in_secs: sms::CODE_VALID_SECS,
            log_mode: state.sms.is_log(),
        }))
    }
    .await;

    tokio::time::sleep_until(deadline).await;
    result
}

async fn reset_confirm(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetConfirmReq>,
) -> Result<Json<OkView>, ApiError> {
    let Some(phone) = crypto::normalize_phone(&req.phone) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };
    let user = find_user_by_phone(&state.store, &phone)
        .ok_or((StatusCode::BAD_REQUEST, "验证码已过期，请重新获取。".to_string()))?;

    validate_password(&req.new_password, &user.username, &phone).map_err(bad_request)?;
    consume_code(&state, Purpose::Reset, &phone, &req.code).await?;

    let salt = crypto::random_bytes(crypto::SALT_LEN);
    let hash = hash_password(req.new_password.clone(), salt.clone(), crypto::PBKDF2_ITERATIONS)
        .await
        .map_err(service_busy)?;

    let uid = user.id.clone();
    state
        .store
        .write(|st| {
            let u = st.users.get_mut(&uid).ok_or("用户不存在。".to_string())?;
            u.password_hash = hash;
            u.password_salt = salt;
            u.pbkdf2_iterations = crypto::PBKDF2_ITERATIONS;
            Ok(())
        })
        .map_err(internal)?;

    // 改密必须吊销所有现有会话：否则"密码被别人改了"之后攻击者的旧 token 还能用。
    revoke_all_sessions(&state, &uid).map_err(internal)?;
    tracing::info!("用户 {uid} 通过短信重置了密码，已吊销全部会话");
    Ok(Json(OkView { ok: true }))
}

/// 已登录状态下改密码。走这条不发短信——正常改密不该消耗短信配额。
async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordReq>,
) -> Result<Json<OkView>, ApiError> {
    let user_id = require_user(&state, &headers)?;
    let user = state
        .store
        .read(|st| st.users.get(&user_id).cloned())
        .ok_or((StatusCode::UNAUTHORIZED, "登录已过期，请重新登录。".to_string()))?;

    let ok = verify_password(
        req.old_password.clone(),
        user.password_salt.clone(),
        user.pbkdf2_iterations,
        user.password_hash.clone(),
    )
    .await
    .map_err(service_busy)?;
    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "原密码不对。".to_string()));
    }

    validate_password(&req.new_password, &user.username, &user.phone).map_err(bad_request)?;

    let salt = crypto::random_bytes(crypto::SALT_LEN);
    let hash = hash_password(req.new_password.clone(), salt.clone(), crypto::PBKDF2_ITERATIONS)
        .await
        .map_err(service_busy)?;

    state
        .store
        .write(|st| {
            let u = st.users.get_mut(&user_id).ok_or("用户不存在。".to_string())?;
            u.password_hash = hash;
            u.password_salt = salt;
            u.pbkdf2_iterations = crypto::PBKDF2_ITERATIONS;
            Ok(())
        })
        .map_err(internal)?;

    revoke_all_sessions(&state, &user_id).map_err(internal)?;
    Ok(Json(OkView { ok: true }))
}

fn build_session_response(state: &AppState, user_id: &str) -> Result<Json<SessionView>, String> {
    let (token, expires_at) = issue_session(state, user_id)?;
    let user = state
        .store
        .read(|st| st.users.get(user_id).cloned())
        .ok_or_else(|| "用户不存在。".to_string())?;
    let acc = state.store.account(user_id);
    Ok(Json(SessionView {
        ok: true,
        token,
        expires_at,
        user: UserView::from(&user),
        balance: acc.balance,
        activated: acc.activated,
    }))
}

// ---------------------------------------------------------------- 错误辅助

fn bad_request(msg: String) -> ApiError {
    (StatusCode::BAD_REQUEST, msg)
}
fn too_many(msg: String) -> ApiError {
    (StatusCode::TOO_MANY_REQUESTS, msg)
}
fn service_busy(msg: String) -> ApiError {
    (StatusCode::SERVICE_UNAVAILABLE, msg)
}
fn internal(msg: String) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, msg)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/auth/register/begin", post(register_begin))
        .route("/v1/auth/register/resend", post(register_resend))
        .route("/v1/auth/register/verify", post(register_verify))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/me", get(me))
        .route("/v1/auth/password/change", post(change_password))
        .route("/v1/auth/reset/begin", post(reset_begin))
        .route("/v1/auth/reset/confirm", post(reset_confirm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_normalization_folds_fullwidth_and_case() {
        // 全角冒充半角是最现实的同形攻击：ａｄｍｉｎ 和 admin 肉眼几乎一样
        assert_eq!(normalize_username_key("ＡＤＭＩＮ"), "admin");
        assert_eq!(normalize_username_key("Admin"), "admin");
        assert_eq!(normalize_username_key("灵魂灯笼"), "灵魂灯笼");
    }

    #[test]
    fn username_validation_allows_chinese_rejects_invisible() {
        assert!(validate_username("小明").is_ok());
        assert!(validate_username("Steve_2026").is_ok());
        assert!(validate_username("a").is_err(), "太短");
        assert!(validate_username(&"x".repeat(25)).is_err(), "太长");
        assert!(validate_username("有 空格").is_err());
        // 零宽空格：肉眼看不出来，能用来伪造别人的用户名
        assert!(validate_username("admin\u{200B}").is_err());
    }

    #[test]
    fn password_policy() {
        assert!(validate_password("Str0ngPass!", "steve", "13800138000").is_ok());
        assert!(validate_password("short1", "steve", "").is_err(), "太短");
        assert!(validate_password("12345678", "steve", "").is_err(), "在弱口令表里");
        assert!(validate_password("aaaaaaaa", "steve", "").is_err(), "同一个字符重复");
        assert!(
            validate_password("13800138000x", "steve", "13800138000").is_err(),
            "不能包含手机号"
        );
    }

    #[test]
    fn rate_limiter_windows() {
        let rl = RateLimiter::default();
        assert!(rl.check("k", 2, 60).is_ok());
        assert!(rl.check("k", 2, 60).is_ok());
        assert!(rl.check("k", 2, 60).is_err(), "第三次该被拦");
        rl.refund("k");
        assert!(rl.check("k", 2, 60).is_ok(), "退还一次之后应该又能过");
    }

    #[test]
    fn rate_limiter_sweep_drops_stale_keys() {
        let rl = RateLimiter::default();
        rl.check("old", 5, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        rl.sweep(1);
        assert_eq!(rl.count("old", 1), 0, "过期的 key 该被清掉，否则内存无限涨");
    }

    #[test]
    fn pending_keys_are_namespaced_by_purpose() {
        // 这是那条"永久锁死找回密码"攻击的回归测试：如果两种流程共用一个 key，
        // register/begin 就能覆盖掉受害者正在进行的 reset。
        let a = pending_key(Purpose::Register, "13800138000");
        let b = pending_key(Purpose::Reset, "13800138000");
        assert_ne!(a, b);
    }
}
