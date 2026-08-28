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

async fn get_account(path: &str, query: &[(&str, &str)]) -> Result<AccountView, String> {
    let resp = client()
        .get(format!("{}{path}", server_base()))
        .query(query)
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_account_response(resp).await
}

async fn post_account(path: &str, body: &serde_json::Value) -> Result<AccountView, String> {
    let resp = client()
        .post(format!("{}{path}", server_base()))
        .json(body)
        .send()
        .await
        .map_err(describe_connect_err)?;
    parse_account_response(resp).await
}

async fn parse_account_response(resp: reqwest::Response) -> Result<AccountView, String> {
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(if detail.is_empty() { "服务器拒绝了这次请求。".to_string() } else { detail });
    }
    resp.json().await.map_err(|e| format!("解析服务器响应失败：{e}"))
}

pub async fn balance(device_id: &str) -> Result<AccountView, String> {
    get_account("/v1/balance", &[("device_id", device_id)]).await
}

pub async fn activate(device_id: &str, license_key: &str) -> Result<AccountView, String> {
    post_account(
        "/v1/activate",
        &serde_json::json!({ "device_id": device_id, "license_key": license_key }),
    )
    .await
}

pub async fn topup(device_id: &str, coins: i64) -> Result<AccountView, String> {
    post_account("/v1/topup", &serde_json::json!({ "device_id": device_id, "coins": coins })).await
}

// 注意：不能加 #[serde(rename_all = "camelCase")]——服务器那边
// （server/src/main.rs 的 AiGenerateReq）用的是原样 snake_case 字段名，
// 没有做驼峰转换。两边字段名对不上，服务器会直接报"缺字段"（真的踩过一次，
// 靠 tests/remote_integration.rs 的真实 HTTP 往返测试才抓出来——如果只是
// 各自测各自的序列化/反序列化，两边"看起来都对"，问题只在两者对接的地方）。
#[derive(Serialize)]
struct AiGenerateReq<'a> {
    device_id: &'a str,
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
    device_id: &str,
    system_prompt: &str,
    user_text: &str,
    model: Option<&str>,
    version: &str,
    history: &[ChatTurn],
) -> Result<AiGenerateResp, String> {
    let resp = client()
        .post(format!("{}/v1/ai/generate", server_base()))
        .json(&AiGenerateReq { device_id, system_prompt, user_text, history, model, version })
        .send()
        .await
        .map_err(describe_connect_err)?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(if detail.is_empty() { "服务器拒绝了这次请求。".to_string() } else { detail });
    }
    resp.json().await.map_err(|e| format!("解析服务器响应失败：{e}"))
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
