//! 灵魂灯笼账本 + AI 调用代理服务。
//!
//! 存在的理由：客户端此前把大模型 key 直接编译进分发出去的软件包里，任何拿到
//! 安装包的人理论上都能把 key 抠出来盗刷（这件事已经真实发生过一次）。这个
//! 服务把"调用大模型"这一步挪到这里——key 只活在这台服务器的环境变量里，
//! 客户端只发"我要生成什么"，从来碰不到能直接花钱的凭证。
//!
//! 顺带把余额/激活码也一起收到这里管理：本地文件版本再怎么加签名校验，
//! 面对"用户对自己的文件有完全读写权限"这个前提都是治标不治本，真正的
//! 权威数据只能放在用户碰不到的地方。
//!
//! 部署环境：国内机房裸 IP，没有域名/备案，所以这里不走公共 CA 签发的证书，
//! 而是自签名证书 + 客户端证书锁定（client 端只信任这一张证书，不信任系统
//! 公共信任链）。证书生成脚本见 server/certs/README.md。

mod ai_proxy;
mod ledger;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use ledger::{Account, Ledger};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

struct AppState {
    ledger: Ledger,
    ai: ai_proxy::AiConfig,
}

/// 账户信息的对外形状——不透出 redeemed_keys（客户端不需要知道具体兑换过
/// 哪些码，只需要知道当前余额和是否已激活）。
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

type ApiError = (StatusCode, String);

fn check_device_id(device_id: &str) -> Result<(), ApiError> {
    if ledger::is_valid_device_id(device_id) {
        Ok(())
    } else {
        Err((StatusCode::BAD_REQUEST, "device_id 无效。".to_string()))
    }
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct DeviceQuery {
    device_id: String,
}

async fn get_balance(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DeviceQuery>,
) -> Result<Json<AccountView>, ApiError> {
    check_device_id(&q.device_id)?;
    Ok(Json(state.ledger.snapshot(&q.device_id).into()))
}

#[derive(Deserialize)]
struct ActivateReq {
    device_id: String,
    license_key: String,
}

async fn activate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ActivateReq>,
) -> Result<Json<AccountView>, ApiError> {
    check_device_id(&req.device_id)?;
    state
        .ledger
        .activate(&req.device_id, &req.license_key)
        .map(|a| Json(a.into()))
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Deserialize)]
struct TopupReq {
    device_id: String,
    coins: i64,
}

async fn topup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TopupReq>,
) -> Result<Json<AccountView>, ApiError> {
    check_device_id(&req.device_id)?;
    state
        .ledger
        .topup(&req.device_id, req.coins)
        .map(|a| Json(a.into()))
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Deserialize)]
struct AiGenerateReq {
    device_id: String,
    system_prompt: String,
    user_text: String,
    #[serde(default)]
    history: Vec<ai_proxy::ChatTurn>,
    /// 客户端想用哪个模型；留空/不传就退回 .env 里 AI_MODEL 配置的默认值。
    /// 不接受客户端指定 endpoint/key——那两个是真正的凭证，模型名只是个
    /// "选哪档价格/能力"的偏好，交给客户端选没有安全问题。
    #[serde(default)]
    model: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiGenerateResp {
    ok: bool,
    content: Option<String>,
    error: Option<String>,
    usage: Option<ai_proxy::AiUsage>,
    balance: i64,
}

async fn ai_generate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AiGenerateReq>,
) -> Result<Json<AiGenerateResp>, ApiError> {
    check_device_id(&req.device_id)?;

    let account = state.ledger.snapshot(&req.device_id);
    if account.balance <= 0 {
        return Ok(Json(AiGenerateResp {
            ok: false,
            content: None,
            error: Some("余额不足，请先激活 / 充值。".to_string()),
            usage: None,
            balance: account.balance,
        }));
    }

    let model = req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&state.ai.model);

    match ai_proxy::call_upstream(&state.ai, model, &req.system_prompt, &req.user_text, req.history).await {
        Ok((content, usage)) => {
            // 成功才扣费，按真实 token 用量折算，跟此前客户端版本的原则一致。
            let coins = ai_proxy::coins_to_charge(model, usage.as_ref());
            let after = state.ledger.consume(&req.device_id, coins);
            Ok(Json(AiGenerateResp {
                ok: true,
                content: Some(content),
                error: None,
                usage,
                balance: after.balance,
            }))
        }
        Err(e) => Ok(Json(AiGenerateResp {
            ok: false,
            content: None,
            error: Some(e),
            usage: None,
            balance: account.balance,
        })),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // reqwest（出站调 AI 上游）和 axum-server（入站 TLS）都传递依赖了 rustls，
    // 但都没有在同一个进程里唯一确定该用哪个加密后端（ring / aws-lc-rs），
    // 不显式装一个默认的，启动就会直接 panic（已经踩过一次）。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("安装 rustls 默认加密后端失败（正常情况下只会调用一次，失败基本不可能发生）");

    let ledger_path = std::env::var("LEDGER_PATH").unwrap_or_else(|_| "ledger.json".to_string());
    let ledger = Ledger::load_or_default(ledger_path.into());
    let ai = ai_proxy::AiConfig::from_env();

    let state = Arc::new(AppState { ledger, ai });

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/balance", get(get_balance))
        .route("/v1/activate", post(activate))
        .route("/v1/topup", post(topup))
        .route("/v1/ai/generate", post(ai_generate))
        .with_state(state);

    let cert_path = std::env::var("TLS_CERT").unwrap_or_else(|_| "certs/server.crt".to_string());
    let key_path = std::env::var("TLS_KEY").unwrap_or_else(|_| "certs/server.key".to_string());
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .unwrap_or_else(|e| panic!("加载证书失败（{cert_path} / {key_path}）：{e}"));

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8443".to_string());
    let addr: SocketAddr = bind_addr.parse().unwrap_or_else(|e| panic!("BIND_ADDR 格式不对（{bind_addr}）：{e}"));

    tracing::info!("灵魂灯笼服务监听 {addr}");
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
