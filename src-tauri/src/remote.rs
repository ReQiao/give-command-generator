//! 连接自建的账本 + AI 调用代理服务器（仓库根目录 server/）。
//!
//! 存在的理由：这个客户端此前把大模型 key 直接编译进分发出去的软件包里，
//! 被人拆出来盗刷过一次真实发生的事故。现在真实 key 只活在服务器的环境
//! 变量里，这个模块只发"我要生成什么"，从来碰不到能直接花钱的凭证。
//! 余额/激活码也一并收进了服务器（见 server/src/ledger.rs），这里不再有
//! 本地的"权威余额"这回事——本模块的返回值就是当前唯一作数的答案。
//!
//! 走的是自签名证书 + 客户端证书锁定，不是公共 CA——服务器部署在国内裸 IP
//! 机房，没有域名/备案。`tls_built_in_root_certs(false)` + `add_root_certificate`
//! 这个组合是真正意义上的锁定："只信任这一张，其它一律拒绝"，不是"额外多
//! 信任一个"；已经用真实 TLS 握手验证过这个组合能正确工作
//! （见 server/tests/tls_pinning.rs）。

use crate::ai::{AiUsage, ChatTurn};
use crate::session;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// 服务器地址，含端口。必须和证书的 IP SAN 是同一个 IP，否则握手直接失败
/// （这是设计如此——对不上就该拒绝，不能悄悄放行）。
const SERVER_BASE: &str = "https://120.26.175.121:8443";

/// 测试逃生舱，同 `pinned_cert_pem`：设了这个环境变量就用它代替内置地址，
/// 让 tests/ 下的集成测试能指向本地起的临时测试服务器，而不必连生产地址。
fn server_base() -> String {
    std::env::var("SOUL_LANTERN_SERVER_BASE").unwrap_or_else(|_| SERVER_BASE.to_string())
}

/// 锁定的证书公钥（PEM），来自服务器上 generate.sh 现场生成的 server.crt
/// （私钥留在服务器上，从未经过这里）。已核对 SAN 是 IP:120.26.175.121、
/// basicConstraints 是 CA:FALSE（不是早前踩过的 CaUsedAsEndEntity 那个坑）。
const PINNED_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIBvTCCAWKgAwIBAgIUVhcCDUQFkiyA4NZdNFAz+LmngucwCgYIKoZIzj0EAwIw
GTEXMBUGA1UEAwwOMTIwLjI2LjE3NS4xMjEwHhcNMjYwODEwMTIxNTAyWhcNNDYw
ODA1MTIxNTAyWjAZMRcwFQYDVQQDDA4xMjAuMjYuMTc1LjEyMTBZMBMGByqGSM49
AgEGCCqGSM49AwEHA0IABKTtZ2fmCxZyqYSp8UTNAr1FcH8dW4OMsyCq8jeDVX1F
qX/MUmhP4VaQ7vli6Y3BpFfPlYFKY2tlr7lEz47ScSijgYcwgYQwHQYDVR0OBBYE
FGW0U9WJSAUO/CWlP8QP+SEZTFe4MB8GA1UdIwQYMBaAFGW0U9WJSAUO/CWlP8QP
+SEZTFe4MA8GA1UdEQQIMAaHBHgar3kwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8E
BAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwCgYIKoZIzj0EAwIDSQAwRgIhAOmb
AGZkXgptheFQ73NyscnWdNew9Pv5CJW5IXAqndn0AiEA+Bo5zYfdCNEPw62aMkrH
qXn++VziEGlP1US9zfaXJw0=
-----END CERTIFICATE-----"#;

/// 测试 / 联调用的逃生舱：设了这个环境变量就读文件内容代替内置的
/// `PINNED_CERT_PEM`，正常发布的客户端不会设这个变量，走的还是编译进去的
/// 那份。这里存在的唯一理由是 tests/ 下的集成测试要用一张现场生成的临时
/// 证书验证整条链路，不能也不该为了测试去改动打包进正式客户端的那个常量。
fn pinned_cert_pem() -> String {
    std::env::var("SOUL_LANTERN_PINNED_CERT_FILE")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| PINNED_CERT_PEM.to_string())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // reqwest 传递依赖了 rustls 但不保证唯一确定加密后端，不显式装一个
        // 默认的，第一次真正握手时会直接 panic（服务端那边已经踩过这个坑，
        // 见 server/src/main.rs 的同一行注释）。重复调用不会报错，忽略返回值。
        let _ = rustls::crypto::ring::default_provider().install_default();

        let cert = reqwest::Certificate::from_pem(pinned_cert_pem().as_bytes())
            .expect("内置证书 PEM 解析失败——多半是占位符还没换成真实证书内容");

        reqwest::Client::builder()
            .tls_built_in_root_certs(false) // 只信任下面这一张，不信任系统公共信任链
            .add_root_certificate(cert)
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("构建 HTTPS 客户端失败")
    })
}

fn describe_connect_err(e: reqwest::Error) -> String {
    format!("无法连接服务器：{e}")
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub activated: bool,
    pub balance: i64,
}

/// 所有需要登录的请求都从这里出去。
///
/// 没有本地 token 就直接返回"请先登录"，不发这一趟网络请求——省一次往返，
/// 也让"未登录"和"服务器连不上"两种情况在 UI 上不会混成同一句话。
fn bearer() -> Result<String, String> {
    session::token().ok_or_else(|| "请先登录。".to_string())
}

/// 统一处理响应。
///
/// 401 在这里集中清掉本地会话，而不是让每个调用点各写一遍——漏掉一处就会
/// 出现"服务器早就不认这个 token 了，客户端还一直拿它去撞"的状态。
async fn parse_json<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        session::clear();
        let detail = resp.text().await.unwrap_or_default();
        return Err(if detail.is_empty() { "登录已过期，请重新登录。".to_string() } else { detail });
    }
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(if detail.is_empty() { "服务器拒绝了这次请求。".to_string() } else { detail });
    }
    resp.json().await.map_err(|e| format!("解析服务器响应失败：{e}"))
}

async fn get_auth<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let token = bearer()?;
    let resp = client()
        .get(format!("{}{path}", server_base()))
        .bearer_auth(token)
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

async fn post_auth<T: serde::de::DeserializeOwned>(
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    let token = bearer()?;
    let resp = client()
        .post(format!("{}{path}", server_base()))
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

/// 不需要登录的 POST（注册、登录、找回密码这几个本来就是登录之前的动作）。
async fn post_public<T: serde::de::DeserializeOwned>(
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    let resp = client()
        .post(format!("{}{path}", server_base()))
        .json(body)
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

// ---------------------------------------------------------------- 账号
//
// 请求体一律 snake_case、**不加** rename_all；响应体一律 camelCase、加 rename_all。
// 这不是风格洁癖，是和服务端 server/src/auth.rs 对齐的硬约定——两边字段名对不上时，
// 各自的单测都是绿的，只有 tests/remote_integration.rs 那种真实 HTTP 往返才抓得到
// （下面 AiGenerateReq 上那条注释记的就是真踩过的那一次）。

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub username: String,
    pub phone_masked: String,
    pub created_at: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub token: String,
    pub expires_at: u64,
    pub user: UserView,
    pub balance: i64,
    pub activated: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MeView {
    pub user: UserView,
    pub balance: i64,
    pub activated: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CodeSentView {
    pub ok: bool,
    pub phone_masked: String,
    pub expires_in_secs: u32,
    /// 服务端 SMS_KIND=log 时为 true。界面上要提示"当前是日志模式，
    /// 短信不会真的发出"，否则用户会一直等一条永远不会来的短信。
    pub log_mode: bool,
    /// 短信开头方括号里那个签名，由服务端下发。
    /// `#[serde(default)]` 是给老服务端留的：它不发这个字段，反序列化不能因此失败。
    #[serde(default)]
    pub sign_name: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionView {
    /// 服务端的逃生开关：短信通道整个挂掉时把它设成 false，客户端就不挡登录门禁。
    pub auth_required: bool,
    pub min_client: String,
    /// 短信签名。老服务端不发，所以是可选的。
    #[serde(default)]
    pub sms_sign_name: Option<String>,
}

pub async fn register_begin(username: &str, password: &str, phone: &str) -> Result<CodeSentView, String> {
    post_public(
        "/v1/auth/register/begin",
        &serde_json::json!({ "username": username, "password": password, "phone": phone }),
    )
    .await
}

pub async fn register_resend(phone: &str) -> Result<CodeSentView, String> {
    post_public("/v1/auth/register/resend", &serde_json::json!({ "phone": phone })).await
}

pub async fn register_verify(phone: &str, code: &str) -> Result<SessionView, String> {
    post_public("/v1/auth/register/verify", &serde_json::json!({ "phone": phone, "code": code })).await
}

pub async fn login(account: &str, password: &str) -> Result<SessionView, String> {
    post_public("/v1/auth/login", &serde_json::json!({ "account": account, "password": password })).await
}

pub async fn logout() -> Result<(), String> {
    // 本地会话无论如何都要清掉：就算服务器这次没连上，用户点了"退出登录"
    // 就该在这台机器上退出，不能因为网络问题把人留在登录态。
    let result: Result<serde_json::Value, String> = post_auth("/v1/auth/logout", &serde_json::json!({})).await;
    session::clear();
    result.map(|_| ())
}

pub async fn me() -> Result<MeView, String> {
    get_auth("/v1/auth/me").await
}

pub async fn change_password(old_password: &str, new_password: &str) -> Result<(), String> {
    let _: serde_json::Value = post_auth(
        "/v1/auth/password/change",
        &serde_json::json!({ "old_password": old_password, "new_password": new_password }),
    )
    .await?;
    // 服务端改密会吊销所有会话，本地这份也就作废了
    session::clear();
    Ok(())
}

pub async fn reset_begin(phone: &str) -> Result<CodeSentView, String> {
    post_public("/v1/auth/reset/begin", &serde_json::json!({ "phone": phone })).await
}

pub async fn reset_confirm(phone: &str, code: &str, new_password: &str) -> Result<(), String> {
    let _: serde_json::Value = post_public(
        "/v1/auth/reset/confirm",
        &serde_json::json!({ "phone": phone, "code": code, "new_password": new_password }),
    )
    .await?;
    Ok(())
}

pub async fn server_version() -> Result<VersionView, String> {
    let resp = client()
        .get(format!("{}/v1/version", server_base()))
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

// ---------------------------------------------------------------- 账本

pub async fn balance() -> Result<AccountView, String> {
    get_auth("/v1/balance").await
}

pub async fn activate(license_key: &str) -> Result<AccountView, String> {
    post_auth("/v1/activate", &serde_json::json!({ "license_key": license_key })).await
}

pub async fn topup(coins: i64) -> Result<AccountView, String> {
    post_auth("/v1/topup", &serde_json::json!({ "coins": coins })).await
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopupTier {
    pub yuan: f64,
    pub coins: i64,
}

/// 充值档位改成从服务器拉：以前客户端和服务端各维护一份静态表，改一边忘另一边
/// 就会出现"界面上写着 5 元 5100 币、点下去服务器说这不是预设档位"。
pub async fn topup_tiers() -> Result<Vec<TopupTier>, String> {
    let resp = client()
        .get(format!("{}/v1/topup/tiers", server_base()))
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

// 注意：不能加 #[serde(rename_all = "camelCase")]——服务器那边
// （server/src/main.rs 的 AiGenerateReq）用的是原样 snake_case 字段名，
// 没有做驼峰转换。两边字段名对不上，服务器会直接报"缺字段"（真的踩过一次，
// 靠 tests/remote_integration.rs 的真实 HTTP 往返测试才抓出来——如果只是
// 各自测各自的序列化/反序列化，两边"看起来都对"，问题只在两者对接的地方）。
#[derive(Serialize)]
struct AiGenerateReq<'a> {
    system_prompt: &'a str,
    user_text: &'a str,
    history: &'a [ChatTurn],
    /// 留空/不传时服务器退回 .env 里 AI_MODEL 配置的默认值（见
    /// server/src/main.rs::ai_generate）。endpoint/key 依然只由服务器决定，
    /// 不接受客户端指定——那两个是真正的凭证，模型名只是"选哪档价格/能力"
    /// 的偏好，交给用户选没有安全问题。
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    /// 目标 Minecraft 版本字符串（如 "java_1_21_11_plus"/"bedrock"），原样
    /// 透传给服务器——`give::dispatch`/`give::builder` 现在跑在服务器上，
    /// 版本感知的目录校验/指令构建都需要这个信息（见迁移计划）。这里不用
    /// 客户端自己的 GiveVersion 类型，直接收一个前端已经算好的字符串即可，
    /// 客户端 Rust 侧不需要再理解这个枚举的具体取值。
    version: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerateResp {
    pub ok: bool,
    pub commands: Vec<String>,
    pub loop_commands: Vec<String>,
    pub failures: Vec<String>,
    pub explanation: String,
    pub error: Option<String>,
    pub usage: Option<AiUsage>,
    /// 供客户端存入多轮对话历史使用，不在 UI 展示。
    pub raw_content: Option<String>,
    pub balance: i64,
}

pub async fn ai_generate(
    system_prompt: &str,
    user_text: &str,
    model: Option<&str>,
    version: &str,
    history: &[ChatTurn],
) -> Result<AiGenerateResp, String> {
    let token = bearer()?;
    let resp = client()
        .post(format!("{}/v1/ai/generate", server_base()))
        .bearer_auth(token)
        .json(&AiGenerateReq { system_prompt, user_text, history, model, version })
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_json(resp).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 防的是"占位符忘了换成真实证书"或者"贴的时候手滑改坏了"这两种情况——
    /// 光靠人眼看 PEM 那一长串 base64 是看不出来对不对的，跑这条测试
    /// 至少能保证它现在是一份能被 reqwest 正常解析的合法证书。
    #[test]
    fn pinned_cert_is_a_valid_parseable_certificate() {
        reqwest::Certificate::from_pem(PINNED_CERT_PEM.as_bytes())
            .expect("PINNED_CERT_PEM 应该是一份合法证书——是不是还是占位符，或者粘贴时手滑改坏了？");
    }

    /// 证书内容长度做个粗筛：早前的占位符字符串远比一份真实证书短。
    /// SERVER_BASE 打的地址和证书 SAN 是否匹配这件事，靠的是
    /// tests/remote_integration.rs 那条真实 TLS 握手的集成测试来验证——
    /// 单测这里只检查"这不再是那句占位符英文"这种低成本但有效的粗筛。
    #[test]
    fn pinned_cert_is_not_the_placeholder() {
        assert!(
            PINNED_CERT_PEM.len() > 200,
            "证书内容看起来太短，多半还是占位符没换成真实证书"
        );
        assert!(
            !PINNED_CERT_PEM.contains("REPLACE_WITH_REAL"),
            "PINNED_CERT_PEM 还是占位符文本，没有换成真实证书内容"
        );
    }
}
