//! 灵魂灯笼服务：账号 + 账本 + AI 调用代理。
//!
//! 存在的理由：客户端此前把大模型 key 直接编译进分发出去的软件包里，任何拿到
//! 安装包的人理论上都能把 key 抠出来盗刷（这件事已经真实发生过一次）。这个
//! 服务把"调用大模型"这一步挪到这里——key 只活在这台服务器的环境变量里，
//! 客户端只发"我要生成什么"，从来碰不到能直接花钱的凭证。
//!
//! 余额/激活码也一起收到这里管理：本地文件版本再怎么加签名校验，面对"用户对
//! 自己的文件有完全读写权限"这个前提都是治标不治本。
//!
//! 这一版新增了**账号体系**（手机号验证码注册 / 用户名密码登录 / 短信找回密码），
//! 并且删掉了此前的匿名 device_id 身份。老的 device_id 有两个致命问题：余额不能
//! 跨设备找回；而且账本的每个写入口都是 `entry(key).or_insert_with(fresh)`，
//! 对任何没见过的 key 都会凭空发欢迎币——删掉本地那个文件就能无限刷。
//!
//! 部署环境：国内机房裸 IP，没有域名/备案，所以这里不走公共 CA 签发的证书，
//! 而是自签名证书 + 客户端证书锁定。证书生成脚本见 server/certs/README.md。
//! 短信同理走的是阿里云「号码认证服务」的免资质通道，见 sms.rs 顶部注释。

mod ai_proxy;
mod auth;
mod crypto;
mod give;
mod ledger;
mod sms;
mod store;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use ledger::Account;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use store::Store;

pub struct AppState {
    pub store: Store,
    pub ai: ai_proxy::AiConfig,
    pub sms: sms::Sender,
    pub rate_limiter: auth::RateLimiter,
    /// 只活在环境变量里的服务端密钥：会话 token 摘要、验证码摘要、激活码校验位
    /// 都靠它。落盘的东西即便被人拿走（备份文件、服务器被入侵读到磁盘），
    /// 没有 pepper 也反查不回原值。
    pub auth_pepper: Vec<u8>,
    /// 未设置时整个 admin 路由**不注册**——不是返回 403，而是让攻击者连路径
    /// 存不存在都探不到。
    pub admin_token: Option<String>,
}

pub type ApiError = (StatusCode, String);

/// 账户信息的对外形状——不透出 redeemed_keys（客户端不需要知道具体兑换过哪些码）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    activated: bool,
    balance: i64,
}

impl From<Account> for AccountView {
    fn from(a: Account) -> Self {
        AccountView { activated: a.activated, balance: a.balance }
    }
}

async fn health() -> &'static str {
    "ok"
}

/// 客户端启动时拉一次，用来判断"服务端认不认这个版本的客户端"。
/// `authRequired` 是个逃生开关：万一短信通道整个挂了，把它设成 false 能让客户端
/// 暂时不挡在登录门禁后面，不用重新发一版安装包。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionView {
    auth_required: bool,
    min_client: String,
}

async fn version() -> Json<VersionView> {
    Json(VersionView {
        auth_required: std::env::var("AUTH_REQUIRED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true),
        min_client: std::env::var("MIN_CLIENT").unwrap_or_else(|_| "0.0.0".to_string()),
    })
}

async fn get_balance(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AccountView>, ApiError> {
    let user_id = auth::require_user(&state, &headers)?;
    Ok(Json(state.store.account(&user_id).into()))
}

#[derive(Deserialize)]
struct ActivateReq {
    license_key: String,
}

async fn activate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ActivateReq>,
) -> Result<Json<AccountView>, ApiError> {
    let user_id = auth::require_user(&state, &headers)?;
    // 兑换码是可以爆破的东西（虽然加了 HMAC 校验位，但没有速率限制的话
    // 攻击者仍可以拿它当在线预言机）。一天 10 次对正常用户绰绰有余。
    if state.rate_limiter.check(&format!("activate:{user_id}"), 10, 24 * 3600).is_err() {
        return Err((StatusCode::TOO_MANY_REQUESTS, "今天兑换尝试次数太多了，请明天再试。".into()));
    }
    let acc = state
        .store
        .activate(&user_id, &req.license_key, &state.auth_pepper)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    tracing::info!("用户 {user_id} 兑换激活码成功，余额 {}", acc.balance);
    Ok(Json(acc.into()))
}

#[derive(Deserialize)]
struct TopupReq {
    coins: i64,
}

/// 充值。
///
/// **注意这个接口现在仍然是"点了就免费到账"**（还没接真实支付网关）。改造前它是
/// 完全无鉴权的——任何人 `curl -d '{"device_id":"x","coins":888888}'` 就能给自己
/// 发最大档。现在至少要求登录、限流、并且每一次都记审计日志，
/// 等接了真实支付网关再把免费发币这段拿掉。
async fn topup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TopupReq>,
) -> Result<Json<AccountView>, ApiError> {
    let user_id = auth::require_user(&state, &headers)?;
    if state.rate_limiter.check(&format!("topup:{user_id}"), 20, 24 * 3600).is_err() {
        return Err((StatusCode::TOO_MANY_REQUESTS, "今天充值次数太多了，请明天再试。".into()));
    }
    let acc = state.store.topup(&user_id, req.coins).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    tracing::info!("【审计】免费充值 user_id={user_id} coins={} 余额={}", req.coins, acc.balance);
    Ok(Json(acc.into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopupTierView {
    yuan: f64,
    coins: i64,
}

/// 充值档位由服务器给，客户端不再各自维护一份静态表。
async fn topup_tiers() -> Json<Vec<TopupTierView>> {
    Json(
        ledger::TOPUP_TIERS
            .iter()
            .map(|&(yuan, coins)| TopupTierView { yuan, coins })
            .collect(),
    )
}

#[derive(Deserialize)]
struct AiGenerateReq {
    system_prompt: String,
    user_text: String,
    #[serde(default)]
    history: Vec<ai_proxy::ChatTurn>,
    /// 客户端想用哪个模型；留空/不传就退回 .env 里 AI_MODEL 配置的默认值。
    /// 不接受客户端指定 endpoint/key——那两个是真正的凭证，模型名只是个
    /// "选哪档价格/能力"的偏好，交给客户端选没有安全问题。
    #[serde(default)]
    model: Option<String>,
    /// 目标 Minecraft 版本（Java 各分档 / 基岩）。dispatch 层要按版本选
    /// Java/基岩两套目录做存在性校验、给各构建器注入正确的语法分支。
    version: give::builder::GiveVersion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiGenerateResp {
    ok: bool,
    commands: Vec<String>,
    loop_commands: Vec<String>,
    failures: Vec<String>,
    explanation: String,
    error: Option<String>,
    usage: Option<ai_proxy::AiUsage>,
    raw_content: Option<String>,
    balance: i64,
}

impl AiGenerateResp {
    fn failure(error: impl Into<String>, balance: i64) -> Self {
        AiGenerateResp {
            ok: false,
            commands: Vec::new(),
            loop_commands: Vec::new(),
            failures: Vec::new(),
            explanation: String::new(),
            error: Some(error.into()),
            usage: None,
            raw_content: None,
            balance,
        }
    }
}

async fn ai_generate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AiGenerateReq>,
) -> Result<Json<AiGenerateResp>, ApiError> {
    let user_id = auth::require_user(&state, &headers)?;

    let account = state.store.account(&user_id);
    if account.balance <= 0 {
        return Ok(Json(AiGenerateResp::failure("余额不足，请先激活 / 充值。", account.balance)));
    }

    let model = req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&state.ai.model);

    let (content, usage) =
        match ai_proxy::call_upstream(&state.ai, model, &req.system_prompt, &req.user_text, req.history)
            .await
        {
            Ok(pair) => pair,
            Err(e) => return Ok(Json(AiGenerateResp::failure(e, account.balance))),
        };

    // 成功调用大模型才扣费，按真实 token 用量折算——即便下面解析/构建阶段失败，
    // 钱也已经花在真实的大模型调用上了，不能不扣费。
    let coins = ai_proxy::coins_to_charge(model, usage.as_ref());
    let after = match state.store.consume(&user_id, coins) {
        Ok(a) => a,
        Err(e) => {
            // 落盘失败：这次调用的钱扣不下去。宁可放过这一次也不能返回假成功，
            // 但更不能把用户已经花掉的上游调用结果丢了，所以照常返回内容并记警告。
            tracing::error!("扣费落盘失败（本次调用未计费）user_id={user_id} coins={coins}：{e}");
            state.store.account(&user_id)
        }
    };

    let parsed = match give::parse::parse_ai_content(&content) {
        Ok(p) => p,
        Err(e) => {
            let mut resp = AiGenerateResp::failure(e, after.balance);
            resp.usage = usage;
            resp.raw_content = Some(content);
            return Ok(Json(resp));
        }
    };

    let results = give::dispatch::dispatch_intents(parsed.intents, req.version);
    let mut commands = Vec::new();
    let mut loop_commands = Vec::new();
    let mut failures = Vec::new();
    for r in results {
        if let Some(cmd) = r.command {
            if r.r#loop { loop_commands.push(cmd) } else { commands.push(cmd) }
        } else if let Some(err) = r.error {
            failures.push(format!("{}：{}", r.intent.command_name(), err));
        }
    }

    Ok(Json(AiGenerateResp {
        ok: true,
        commands,
        loop_commands,
        failures,
        explanation: parsed.explanation,
        error: None,
        usage,
        raw_content: Some(content),
        balance: after.balance,
    }))
}

// ---------------------------------------------------------------- admin
//
// 只读接口一律 GET + query 参数，不用 POST + JSON body。
// 理由很具体：Windows PowerShell 5.1（RUNBOOK 推荐的那个）把参数传给原生 exe 时
// 会剥掉内层双引号，`curl.exe -d '{"to":"x"}'` 实际发出去的是 `{to:x}`，
// 服务器返回 400。而你需要用这些接口的时刻，恰恰是"用户收不到验证码、
// 我要赶紧定位"的高压时刻，那时候排查一个和"代码写错了"无法区分的错误最要命。

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = &state.admin_token else {
        return Err((StatusCode::NOT_FOUND, "Not Found".to_string()));
    };
    let given = auth::bearer_token(headers).unwrap_or_default();
    if crypto::constant_time_eq(given.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err((StatusCode::NOT_FOUND, "Not Found".to_string()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatsView {
    users: usize,
    accounts: usize,
    sessions: usize,
    pending: usize,
    total_balance: i64,
    sms_sent_today: usize,
    sms_daily_cap: usize,
    /// 磁盘满 / 权限问题的唯一可观测信号。false 就说明有写不进去的情况发生过。
    last_persist_ok: bool,
    last_persist_micros: u64,
    sms_kind: &'static str,
}

async fn admin_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<StatsView>, ApiError> {
    require_admin(&state, &headers)?;
    let (ok, micros) = state.store.persist_health();
    let v = state.store.read(|st| StatsView {
        users: st.users.len(),
        accounts: st.accounts.len(),
        sessions: st.sessions.len(),
        pending: st.pending.len(),
        total_balance: st.accounts.values().map(|a| a.balance).sum(),
        sms_sent_today: 0,
        sms_daily_cap: 0,
        last_persist_ok: ok,
        last_persist_micros: micros,
        sms_kind: if state.sms.is_log() { "log" } else { "aliyun" },
    });
    Ok(Json(StatsView {
        sms_sent_today: state.rate_limiter.count("sms:global", 24 * 3600),
        sms_daily_cap: std::env::var("SMS_DAILY_CAP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(200),
        ..v
    }))
}

#[derive(Deserialize)]
struct LookupQuery {
    q: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LookupView {
    found: bool,
    user_id: Option<String>,
    username: Option<String>,
    phone_masked: Option<String>,
    balance: i64,
    activated: bool,
    created_at: u64,
    last_login_at: u64,
}

async fn admin_lookup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<LookupQuery>,
) -> Result<Json<LookupView>, ApiError> {
    require_admin(&state, &headers)?;
    let key = auth::normalize_username_key(q.q.trim());
    let phone = crypto::normalize_phone(&q.q);
    let user = state.store.read(|st| {
        st.users
            .values()
            .find(|u| u.username_key == key || Some(&u.phone) == phone.as_ref() || u.id == q.q)
            .cloned()
    });
    let Some(user) = user else {
        return Ok(Json(LookupView {
            found: false,
            user_id: None,
            username: None,
            phone_masked: None,
            balance: 0,
            activated: false,
            created_at: 0,
            last_login_at: 0,
        }));
    };
    let acc = state.store.account(&user.id);
    Ok(Json(LookupView {
        found: true,
        user_id: Some(user.id.clone()),
        username: Some(user.username.clone()),
        phone_masked: Some(crypto::mask_phone(&user.phone)),
        balance: acc.balance,
        activated: acc.activated,
        created_at: user.created_at,
        last_login_at: user.last_login_at,
    }))
}

#[derive(Deserialize)]
struct MailTestQuery {
    to: String,
}

/// 发一条测试短信，把上游的原始错误原样透出来。
/// 这个接口替代掉"改代码加日志 → musl 交叉编译 → scp → restart"整个排障循环。
async fn admin_sms_test(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<MailTestQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers)?;
    let Some(phone) = crypto::normalize_phone(&q.to) else {
        return Err((StatusCode::BAD_REQUEST, "手机号格式不正确。".to_string()));
    };
    match state.sms.send(&phone, sms::Purpose::Register).await {
        Ok(code) => Ok(Json(serde_json::json!({
            "ok": true,
            "logMode": state.sms.is_log(),
            "code": code,
        }))),
        Err(e) => Ok(Json(serde_json::json!({ "ok": false, "error": e }))),
    }
}

#[derive(Deserialize)]
struct AdminUserReq {
    user_id: String,
    action: String,
    #[serde(default)]
    coins: i64,
}

/// 唯一一个改状态的 admin 接口，所以保留 POST。
/// RUNBOOK 里要写成"用记事本存 body.json 然后 curl.exe -d '@body.json'"，
/// 别写内联 JSON。
async fn admin_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AdminUserReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers)?;
    let out = match req.action.as_str() {
        "grant" => {
            let acc = state
                .store
                .add_coins(&req.user_id, req.coins)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            tracing::info!("【审计】admin 给 {} 加了 {} 币", req.user_id, req.coins);
            serde_json::json!({ "ok": true, "balance": acc.balance })
        }
        "revoke_sessions" => {
            state
                .store
                .write(|st| {
                    st.sessions.retain(|_, s| s.user_id != req.user_id);
                    Ok(())
                })
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            serde_json::json!({ "ok": true })
        }
        other => {
            return Err((StatusCode::BAD_REQUEST, format!("未知操作 {other}，支持：grant / revoke_sessions")))
        }
    };
    Ok(Json(out))
}

// ---------------------------------------------------------------- 启动

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    // 不能用 fmt::init()：开了 env-filter feature 之后它走的是
    // EnvFilter::from_default_env()，而那个在 RUST_LOG 未设置时**只放行 ERROR**，
    // 会把启动日志和 SMS_KIND=log 的验证码一起静音。
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
}

fn auth_pepper() -> Vec<u8> {
    std::env::var("AUTH_PEPPER")
        .map(|s| s.into_bytes())
        .unwrap_or_else(|_| {
            panic!(
                "缺少环境变量 AUTH_PEPPER，服务无法启动。\n\
                 它用来给会话 token / 验证码 / 激活码校验位做 HMAC，是账号体系的根密钥。\n\
                 生成一个：openssl rand -base64 48\n\
                 生成后写进 /opt/soul-lantern/.env，**不要更换**——换了会让所有人\n\
                 掉线、并且已发出去的激活码全部作废。"
            )
        })
}

/// `--check`：读全部配置、加载账本、验证证书可读，全部通过 exit 0，任何一项失败
/// 打印具体缺什么并 exit 1，**不绑定端口**。
///
/// 升级流程因此可以固定成安全的四步：
///   scp 到 xxx.new → `sudo -u soul-lantern ./xxx.new --check` → 过了才 mv 覆盖 → restart
/// 而不是"覆盖了再 restart，起不来才发现少配了一个环境变量"。
fn run_check() -> i32 {
    println!("== 灵魂灯笼服务自检 ==");
    let mut failed = false;
    let mut check = |name: &str, result: Result<String, String>| match result {
        Ok(detail) => println!("  [OK]   {name}：{detail}"),
        Err(e) => {
            println!("  [FAIL] {name}：{e}");
            failed = true;
        }
    };

    for key in ["AI_ENDPOINT", "AI_MODEL", "AI_API_KEY", "AUTH_PEPPER"] {
        check(
            key,
            std::env::var(key)
                .map(|v| if key.contains("KEY") || key.contains("PEPPER") {
                    format!("已设置（{} 字符）", v.len())
                } else {
                    v
                })
                .map_err(|_| "未设置（缺了服务起不来）".to_string()),
        );
    }

    let sms_kind = std::env::var("SMS_KIND").unwrap_or_else(|_| "log".to_string());
    check("SMS_KIND", Ok(sms_kind.clone()));
    if sms_kind == "aliyun" {
        for key in ["SMS_ACCESS_KEY_ID", "SMS_ACCESS_KEY_SECRET", "SMS_SIGN_NAME"] {
            check(
                key,
                std::env::var(key)
                    .map(|v| if key.contains("SECRET") { format!("已设置（{} 字符）", v.len()) } else { v })
                    .map_err(|_| "未设置（SMS_KIND=aliyun 时必填）".to_string()),
            );
        }
    } else {
        println!("  [WARN] SMS_KIND 不是 aliyun：验证码只会打进日志，不会真的发短信");
    }

    check(
        "ADMIN_TOKEN",
        Ok(match std::env::var("ADMIN_TOKEN") {
            Ok(v) if !v.is_empty() => format!("已设置（{} 字符），admin 路由会注册", v.len()),
            _ => "未设置，admin 路由不会注册（这是允许的）".to_string(),
        }),
    );

    for (name, default) in [("TLS_CERT", "certs/server.crt"), ("TLS_KEY", "certs/server.key")] {
        let path = std::env::var(name).unwrap_or_else(|_| default.to_string());
        check(
            name,
            std::fs::metadata(&path)
                .map(|m| format!("{path}（{} 字节）", m.len()))
                .map_err(|e| format!("{path} 读不到：{e}")),
        );
    }

    let ledger_path = std::env::var("LEDGER_PATH").unwrap_or_else(|_| "ledger.json".to_string());
    let halt = std::path::Path::new(&ledger_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("HALT");
    if halt.exists() {
        check("HALT 标记", Err(format!("{} 存在，服务会拒绝启动", halt.display())));
    } else {
        check("HALT 标记", Ok("不存在".to_string()));
    }

    check(
        "账本",
        match std::fs::read_to_string(&ledger_path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(format!("{ledger_path} 不存在（首次启动会新建）"))
            }
            Err(e) => Err(format!("{ledger_path} 读不到：{e}")),
            Ok(text) => serde_json::from_str::<store::Persisted>(&text)
                .map(|p| {
                    format!(
                        "{ledger_path} 解析正常（schema v{}，{} 个用户，{} 个账户）",
                        p.schema_version,
                        p.users.len(),
                        p.accounts.len()
                    )
                })
                .map_err(|e| format!("{ledger_path} 解析失败：{e}")),
        },
    );

    let dir = std::path::Path::new(&ledger_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let probe = dir.join(".writetest");
    check(
        "工作目录可写",
        std::fs::write(&probe, b"ok")
            .map(|_| {
                let _ = std::fs::remove_file(&probe);
                format!("{} 可写", dir.display())
            })
            .map_err(|e| {
                format!(
                    "{} 不可写：{e}（最常见原因：恢复备份后忘了 chown soul-lantern:soul-lantern）",
                    dir.display()
                )
            }),
    );

    if failed {
        println!("\n自检未通过——**不要**覆盖现有二进制，先把上面 [FAIL] 的项补齐。");
        1
    } else {
        println!("\n自检通过，可以覆盖二进制并 systemctl restart soul-lantern。");
        0
    }
}

/// `--gen-license N`：离线批量出激活码。
/// 码的校验位由 AUTH_PEPPER 决定，所以必须在配好 .env 的这台服务器上生成。
fn run_gen_license(count: usize) -> i32 {
    let pepper = auth_pepper();
    for _ in 0..count.max(1) {
        println!("{}", crypto::generate_license(&pepper));
    }
    0
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--check") {
        init_tracing();
        std::process::exit(run_check());
    }
    if let Some(pos) = args.iter().position(|a| a == "--gen-license") {
        let n = args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(1);
        std::process::exit(run_gen_license(n));
    }

    init_tracing();

    // reqwest（出站调 AI 上游 / 短信接口）和 axum-server（入站 TLS）都传递依赖了
    // rustls，但都没有唯一确定该用哪个加密后端，不显式装一个默认的，启动就会
    // 直接 panic（已经踩过一次）。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("安装 rustls 默认加密后端失败（正常情况下只会调用一次，失败基本不可能发生）");

    let ledger_path = std::env::var("LEDGER_PATH").unwrap_or_else(|_| "ledger.json".to_string());
    let store = Store::load_or_halt(ledger_path.into());
    let ai = ai_proxy::AiConfig::from_env();
    let sms_sender = sms::Sender::from_env();
    let admin_token = std::env::var("ADMIN_TOKEN").ok().filter(|s| !s.is_empty());

    let state = Arc::new(AppState {
        store,
        ai,
        sms: sms_sender,
        rate_limiter: auth::RateLimiter::default(),
        auth_pepper: auth_pepper(),
        admin_token: admin_token.clone(),
    });

    // 限流表是纯内存的滑动窗口；不定期清理的话，攻击者不断变换 key 就能让它无限涨。
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tick.tick().await;
                state.rate_limiter.sweep(24 * 3600);
            }
        });
    }

    let mut app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
        .route("/v1/balance", get(get_balance))
        .route("/v1/activate", post(activate))
        .route("/v1/topup", post(topup))
        .route("/v1/topup/tiers", get(topup_tiers))
        .route("/v1/ai/generate", post(ai_generate))
        .merge(auth::router());

    if admin_token.is_some() {
        tracing::info!("ADMIN_TOKEN 已设置，注册 /v1/admin/* 路由");
        app = app
            .route("/v1/admin/stats", get(admin_stats))
            .route("/v1/admin/lookup", get(admin_lookup))
            .route("/v1/admin/sms-test", get(admin_sms_test))
            .route("/v1/admin/user", post(admin_user));
    }

    let app = app.with_state(state);

    let cert_path = std::env::var("TLS_CERT").unwrap_or_else(|_| "certs/server.crt".to_string());
    let key_path = std::env::var("TLS_KEY").unwrap_or_else(|_| "certs/server.key".to_string());
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .unwrap_or_else(|e| panic!("加载证书失败（{cert_path} / {key_path}）：{e}"));

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8443".to_string());
    let addr: SocketAddr =
        bind_addr.parse().unwrap_or_else(|e| panic!("BIND_ADDR 格式不对（{bind_addr}）：{e}"));

    tracing::info!("灵魂灯笼服务监听 {addr}");
    // 必须是 with_connect_info：限流要按来源 IP 分桶，拿不到对端地址就只剩
    // 手机号维度，攻击者换手机号就能绕过 IP 限流。
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
