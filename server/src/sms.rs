//! 短信验证码：阿里云「号码认证服务（Dypnsapi）」的短信认证功能。
//!
//! ## 为什么是这个产品，而不是常规的「短信服务（SMS）」
//!
//! 常规短信服务要求签名做运营商实名报备，而个人认证用户过不了这一关
//! （阿里云：个人认证的自用资质无法通过签名实名制报备；腾讯云更彻底，
//! 2025-09-18 起不再新增个人认证自用资质）。号码认证服务里的「短信认证」
//! 是个例外：签名和模板是阿里云自己的、已经报备过，开发者只往里填验证码，
//! 所以个人实名认证就能用，不需要营业执照、不需要域名、不需要 ICP 备案。
//!
//! 代价是**签名不可自定义**——用户收到的短信开头方括号里是服务商的名字，
//! 不会出现"灵魂灯笼"。这没法在短信侧解决，只能靠客户端 UI 提前告知用户
//! "验证码短信来自【xxx】，不是垃圾短信"（见 AiPanel.vue 的注册弹窗文案）。
//!
//! ## 为什么手写签名而不是用官方 SDK
//!
//! 阿里云的官方 SDK 覆盖 Java/Python/PHP/Go/.NET/Node.js/C/C++，**没有 Rust**。
//! 但这个接口走的是阿里云经典的 RPC 风格签名，规则完全公开，用已经在依赖树里的
//! `ring`（HMAC-SHA1）+ `base64` + `reqwest` 手写大约 40 行就够了，
//! 比引入一个 JS/Python 运行时或者放弃这条路划算得多。
//!
//! ## 验证码的状态由谁管
//!
//! 走 `##code##` 占位符模式时，**验证码由阿里云生成、存储、判断有效期和是否匹配**，
//! 我们只负责调 `SendSmsVerifyCode` 发、调 `CheckSmsVerifyCode` 验。这直接省掉了
//! 服务端自己维护"验证码哈希 + 过期时间 + 尝试次数"的一整套逻辑，也就顺带消掉了
//! 那套逻辑上的一批攻击面（验证码摘要要不要加 pepper、尝试次数被用来定点骚扰等）。

use std::collections::BTreeMap;

use crate::crypto;

/// 阿里云赠送模板：登录/注册。
pub const TEMPLATE_REGISTER: &str = "100001";
/// 阿里云赠送模板：重置密码。
pub const TEMPLATE_RESET: &str = "100003";

/// 验证码有效期（秒）。同时透传给阿里云（它据此判断过期）和回显给客户端做倒计时。
pub const CODE_VALID_SECS: u32 = 300;
/// 同一手机号两次发送之间的最小间隔（秒），交给阿里云做第一道频控。
/// 我们自己那层限流（auth.rs）更严，这个是兜底。
pub const SEND_INTERVAL_SECS: u32 = 60;
const CODE_LENGTH: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Register,
    Reset,
}

impl Purpose {
    pub fn template(self) -> &'static str {
        match self {
            Purpose::Register => TEMPLATE_REGISTER,
            Purpose::Reset => TEMPLATE_RESET,
        }
    }

    /// pending 表 key 的前缀。必须区分开：如果两种流程共用一个槽位，
    /// 攻击者能用 register/begin 覆盖掉受害者正在进行的 reset，
    /// 循环执行就是永久锁死任意已知手机号的找回密码功能。
    pub fn key_prefix(self) -> &'static str {
        match self {
            Purpose::Register => "register",
            Purpose::Reset => "reset",
        }
    }
}

#[derive(Clone)]
pub struct AliyunConfig {
    pub access_key_id: String,
    pub access_key_secret: String,
    /// 系统赠送签名的名称，从号码认证服务控制台「赠送签名配置」页面复制。
    /// 不能自定义，填错了阿里云会直接拒绝。
    pub sign_name: String,
    /// 接口地址。默认 dypnsapi.aliyuncs.com；留成可配是因为万一阿里云换了地域
    /// 端点，改一个环境变量就行，不用重新编译——这条是被 AI_ENDPOINT 那次
    /// 404 教出来的。
    pub endpoint: String,
}

/// 发信后端。
///
/// `Log` 存在的理由有两个，都不是"凑数的开发选项"：
/// 1. 不开通任何短信服务、不花一分钱就能把整条注册链路端到端跑通并写完集成测试；
/// 2. 上线后短信出问题（余额欠费、接口变更、被限流）时，切一个环境变量 + restart
///    就是救火通道——用户把手机号报给你，你从 journalctl 里捞验证码。
#[derive(Clone)]
pub enum Sender {
    Log,
    Aliyun(AliyunConfig),
}

impl Sender {
    pub fn from_env() -> Self {
        let kind = std::env::var("SMS_KIND").unwrap_or_else(|_| "log".to_string());
        match kind.trim().to_ascii_lowercase().as_str() {
            "aliyun" => {
                let get = |k: &str| {
                    std::env::var(k).unwrap_or_else(|_| {
                        panic!("SMS_KIND=aliyun 时必须设置环境变量 {k}（见 deploy/.env.example）")
                    })
                };
                Sender::Aliyun(AliyunConfig {
                    access_key_id: get("SMS_ACCESS_KEY_ID"),
                    access_key_secret: get("SMS_ACCESS_KEY_SECRET"),
                    sign_name: get("SMS_SIGN_NAME"),
                    endpoint: std::env::var("SMS_ENDPOINT")
                        .unwrap_or_else(|_| "dypnsapi.aliyuncs.com".to_string()),
                })
            }
            "log" => {
                tracing::warn!(
                    "SMS_KIND=log：验证码只会打进日志，不会真的发短信。\
                     这是开发/救火配置，正式环境请设成 aliyun。"
                );
                Sender::Log
            }
            other => panic!("SMS_KIND 只能是 log 或 aliyun，收到的是 {other:?}"),
        }
    }

    pub fn is_log(&self) -> bool {
        matches!(self, Sender::Log)
    }

    /// 发送验证码。
    ///
    /// 返回 `Ok(Some(code))` 表示这是 Log 后端、验证码由我们自己生成（调用方要自己存），
    /// 返回 `Ok(None)` 表示走的是阿里云、码由阿里云托管（调用方不需要存）。
    pub async fn send(&self, phone: &str, purpose: Purpose) -> Result<Option<String>, String> {
        match self {
            Sender::Log => {
                let code: String = (0..CODE_LENGTH)
                    .map(|_| char::from(b'0' + (crypto::random_bytes(1)[0] % 10)))
                    .collect();
                // 用 error! 级别是刻意的：这本来就是异常运行模式，而且要保证
                // 无论 RUST_LOG 怎么配都能在 journalctl 里捞到（默认过滤器最低放行 ERROR）。
                tracing::error!(
                    "【SMS_KIND=log】给 {} 的{}验证码是 {code}（{}秒内有效）——正式环境不该看到这行",
                    crypto::mask_phone(phone),
                    match purpose {
                        Purpose::Register => "注册",
                        Purpose::Reset => "重置密码",
                    },
                    CODE_VALID_SECS,
                );
                Ok(Some(code))
            }
            Sender::Aliyun(cfg) => {
                let mut params = BTreeMap::new();
                params.insert("Action", "SendSmsVerifyCode".to_string());
                params.insert("PhoneNumber", phone.to_string());
                params.insert("SignName", cfg.sign_name.clone());
                params.insert("TemplateCode", purpose.template().to_string());
                // ##code## 占位符 = 让阿里云生成并托管验证码。
                // 如果这里直接传一个我们自己生成的值，CheckSmsVerifyCode 就不认了，
                // 得自己存自己比对——那正是我们要避开的一堆状态管理。
                params.insert("TemplateParam", r###"{"code":"##code##","min":"5"}"###.to_string());
                params.insert("CodeType", "1".to_string()); // 1 = 纯数字
                params.insert("CodeLength", CODE_LENGTH.to_string());
                params.insert("ValidTime", CODE_VALID_SECS.to_string());
                params.insert("Interval", SEND_INTERVAL_SECS.to_string());
                params.insert("DuplicatePolicy", "1".to_string()); // 新码覆盖旧码

                let resp = call(cfg, params).await?;
                interpret_send_error(&resp)?;
                Ok(None)
            }
        }
    }

    /// 校验验证码。Log 后端不走这里（调用方自己比对本地存的码）。
    pub async fn check(&self, phone: &str, code: &str) -> Result<bool, String> {
        match self {
            Sender::Log => Err("Log 后端的验证码由调用方自行比对，不该走到这里".to_string()),
            Sender::Aliyun(cfg) => {
                let mut params = BTreeMap::new();
                params.insert("Action", "CheckSmsVerifyCode".to_string());
                params.insert("PhoneNumber", phone.to_string());
                params.insert("VerifyCode", code.to_string());
                let resp = call(cfg, params).await?;

                if resp.code.as_deref() != Some("OK") {
                    // 校验接口本身失败（参数错、服务未开通等），和"码不对"是两回事
                    return Err(describe_error(&resp));
                }
                let verify_result = resp
                    .model
                    .as_ref()
                    .and_then(|m| m.verify_result.as_deref())
                    .unwrap_or("");
                Ok(verify_result.eq_ignore_ascii_case("PASS"))
            }
        }
    }
}

#[derive(serde::Deserialize, Debug, Default)]
#[serde(default)]
struct ApiResponse {
    #[serde(rename = "Code")]
    code: Option<String>,
    #[serde(rename = "Message")]
    message: Option<String>,
    #[serde(rename = "Success")]
    success: Option<bool>,
    #[serde(rename = "Model")]
    model: Option<ApiModel>,
}

#[derive(serde::Deserialize, Debug, Default)]
#[serde(default)]
struct ApiModel {
    #[serde(rename = "VerifyResult")]
    verify_result: Option<String>,
}

fn describe_error(resp: &ApiResponse) -> String {
    let code = resp.code.as_deref().unwrap_or("(无)");
    let msg = resp.message.as_deref().unwrap_or("(无)");
    format!("阿里云短信接口返回 {code}：{msg}")
}

/// 把阿里云的错误码翻译成能直接给用户看的中文；翻不出来的原样透出，
/// 方便从 journalctl 里对着阿里云文档查。
fn interpret_send_error(resp: &ApiResponse) -> Result<(), String> {
    match resp.code.as_deref() {
        Some("OK") => Ok(()),
        Some("MOBILE_NUMBER_ILLEGAL") => Err("手机号格式不正确。".to_string()),
        Some("BUSINESS_LIMIT_CONTROL") | Some("FREQUENCY_FAIL") => {
            Err("验证码发送太频繁了，请稍后再试。".to_string())
        }
        Some("FUNCTION_NOT_OPENED") => {
            // 这个是部署配置问题，不是用户的错——日志里要能一眼看出来
            tracing::error!("短信认证功能未开通：去阿里云号码认证服务控制台开通「短信认证产品」");
            Err("短信服务暂时不可用，请稍后再试。".to_string())
        }
        _ => {
            let detail = describe_error(resp);
            tracing::error!("发送验证码失败：{detail}");
            Err("验证码发送失败，请稍后重试。".to_string())
        }
    }
}

// ---------------------------------------------------------------- RPC 签名

fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            // 发验证码是用户点了按钮之后同步等待的动作，超时必须短——
            // 与其让用户对着转圈等 30 秒，不如快点失败让他重试。
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("构建短信 HTTP 客户端失败")
    })
}

async fn call(cfg: &AliyunConfig, mut params: BTreeMap<&str, String>) -> Result<ApiResponse, String> {
    params.insert("Version", "2017-05-25".to_string());
    params.insert("Format", "JSON".to_string());
    params.insert("SignatureMethod", "HMAC-SHA1".to_string());
    params.insert("SignatureVersion", "1.0".to_string());
    params.insert("SignatureNonce", crypto::random_token());
    params.insert("Timestamp", utc_timestamp());
    params.insert("AccessKeyId", cfg.access_key_id.clone());

    let signature = sign(&cfg.access_key_secret, "POST", &params);

    // 参数走 form body 而不是 query string：手机号和验证码不该出现在 URL 里
    // （URL 会进各种访问日志）。阿里云 RPC 风格两种都支持，签名算法一样。
    let mut form: Vec<(String, String)> =
        params.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    form.push(("Signature".to_string(), signature));

    let resp = http_client()
        .post(format!("https://{}/", cfg.endpoint))
        .form(&form)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("调用阿里云短信接口失败：{e}");
            "验证码发送失败，请检查网络后重试。".to_string()
        })?;

    let text = resp.text().await.map_err(|e| {
        tracing::error!("读取阿里云短信响应失败：{e}");
        "验证码服务响应异常。".to_string()
    })?;

    serde_json::from_str::<ApiResponse>(&text).map_err(|e| {
        tracing::error!("解析阿里云短信响应失败：{e}；原始响应：{text}");
        "验证码服务响应异常。".to_string()
    })
}

fn utc_timestamp() -> String {
    // 阿里云要的是 ISO8601 UTC，形如 2026-08-14T05:12:33Z。
    // 为这一个格式引入 chrono/time 不划算，手算一次民用历。
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard Hinnant 的 civil_from_days 算法。纯整数运算，不依赖时区库。
fn civil_from_unix(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32)
}

/// 阿里云 RPC 风格签名。
///
/// 规则：
///   待签名串 = HTTPMethod + "&" + encode("/") + "&" + encode(规范化查询串)
///   规范化查询串 = 参数按名字典序排列、逐个 `encode(k)=encode(v)`、用 & 连接
///   Signature = Base64(HMAC-SHA1(AccessKeySecret + "&", 待签名串))
///
/// 参数用 `BTreeMap` 装就是为了拿到"按 key 字典序"这个前提，不用手动排。
fn sign(access_key_secret: &str, method: &str, params: &BTreeMap<&str, String>) -> String {
    use base64::Engine as _;

    let canonical = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let string_to_sign = format!("{method}&{}&{}", percent_encode("/"), percent_encode(&canonical));
    let mac = crypto::hmac_sha1(format!("{access_key_secret}&").as_bytes(), string_to_sign.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac)
}

/// 阿里云要求的 URL 编码，和标准 `encodeURIComponent` 有三处不同：
/// `+` → `%20`、`*` → `%2A`、`%7E` → `~`。这三条是签名对不上的经典原因。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_follows_aliyun_rules() {
        // 未保留字符原样
        assert_eq!(percent_encode("abcXYZ019-_.~"), "abcXYZ019-_.~");
        // 空格必须是 %20 而不是 +
        assert_eq!(percent_encode(" "), "%20");
        // 星号必须编码
        assert_eq!(percent_encode("*"), "%2A");
        // 波浪号必须**不**编码（标准库很多实现会把它编成 %7E，那样签名对不上）
        assert_eq!(percent_encode("~"), "~");
        assert_eq!(percent_encode("/"), "%2F");
        assert_eq!(percent_encode("{\"a\":\"b\"}"), "%7B%22a%22%3A%22b%22%7D");
    }

    /// 固定住签名算法，防止以后重构 `percent_encode` / 排序 / 拼接顺序时悄悄改坏
    /// ——签名一旦不对，阿里云只会回一句 `SignatureDoesNotMatch`，
    /// 从那句话反推是哪一步错了非常费劲，所以这条测试值得存在。
    ///
    /// 期望值的来源要说清楚：它**不是**从阿里云文档上抄下来的常量，而是用一份
    /// 独立的 Python 实现（`hmac`+`hashlib`+`urllib.parse.quote`，照文档描述的
    /// 算法重写一遍）算出来、与本实现比对一致之后固定下来的。也就是说这条测试
    /// 保证的是"两个独立实现对同一份文档的理解一致"，不是"和某个记忆中的
    /// 魔法字符串一致"。参数和密钥都是文档里的占位值，不是真实凭证。
    #[test]
    fn signature_matches_known_vector() {
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("AccessKeyId", "testid".into());
        params.insert("Action", "DescribeRegions".into());
        params.insert("Format", "XML".into());
        params.insert("SignatureMethod", "HMAC-SHA1".into());
        params.insert("SignatureNonce", "3ee8c1b8-83d3-44af-a94f-4e0ad82fd6cf".into());
        params.insert("SignatureVersion", "1.0".into());
        params.insert("Timestamp", "2016-02-23T12:46:24Z".into());
        params.insert("Version", "2014-05-26".into());

        let sig = sign("testsecret", "GET", &params);
        assert_eq!(sig, "OLeaidS1JvxuMvnyHOwuJ+uX5qY=");
    }

    #[test]
    fn timestamp_is_iso8601_utc() {
        let ts = utc_timestamp();
        assert_eq!(ts.len(), 20, "形如 2026-08-14T05:12:33Z，实际 {ts}");
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }

    #[test]
    fn civil_conversion_spot_checks() {
        // 1970-01-01T00:00:00Z
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
        // 2016-02-23T12:46:24Z（闰年 2 月，顺带验一下闰年处理）
        assert_eq!(civil_from_unix(1_456_231_584), (2016, 2, 23, 12, 46, 24));
        // 2000-03-01T00:00:00Z（世纪闰年边界）
        assert_eq!(civil_from_unix(951_868_800), (2000, 3, 1, 0, 0, 0));
    }

    #[test]
    fn purposes_use_distinct_templates_and_prefixes() {
        assert_ne!(Purpose::Register.template(), Purpose::Reset.template());
        // 前缀不同是安全要求，不只是整洁：见 Purpose::key_prefix 的注释
        assert_ne!(Purpose::Register.key_prefix(), Purpose::Reset.key_prefix());
    }

    #[tokio::test]
    async fn log_sender_returns_a_code_for_local_verification() {
        let sender = Sender::Log;
        let code = sender.send("13800138000", Purpose::Register).await.unwrap();
        let code = code.expect("Log 后端应该把码交给调用方自己存");
        assert_eq!(code.len(), CODE_LENGTH as usize);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
