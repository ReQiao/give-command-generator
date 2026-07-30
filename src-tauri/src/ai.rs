//! AI 桥接：调用通义千问（DashScope 的 OpenAI 兼容接口）。
//!
//! 分工：
//!   - 系统提示词由前端用 catalog 构造好后传进来，本模块只负责「注入 key + 联网」。
//!   - API key 留在后端进程：用户填写 > 环境变量 DASHSCOPE_API_KEY > 内置 key。
//!   - 只把 AI 返回的原始 JSON 文本透传回前端；解析成意图、再确定性地构建命令字符串，
//!     全部由前端的 logic/dispatch.ts 完成——后端不碰命令语法。
//!   - 调用成功才扣 1 次余额（失败不扣）。

use crate::billing::Billing;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiUsage {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResponse {
    pub ok: bool,
    /// AI 返回的原始 JSON 文本（成功时）。
    pub content: Option<String>,
    /// 失败原因（失败时）。
    pub error: Option<String>,
    pub usage: Option<AiUsage>,
    /// 当前剩余余额。
    pub balance: i64,
}

impl AiResponse {
    fn err(msg: impl Into<String>, balance: i64) -> Self {
        AiResponse {
            ok: false,
            content: None,
            error: Some(msg.into()),
            usage: None,
            balance,
        }
    }
}

const DEFAULT_ENDPOINT: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
const DEFAULT_MODEL: &str = "qwen-plus";

/// 内置 API key（变现用）。当前为 None——需要变现时再签发并以混淆形式嵌入。
fn builtin_key() -> Option<String> {
    None
}

fn non_blank(value: Option<String>) -> Option<String> {
    value.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// 解析出可用的 API key：用户传入 > 环境变量 > 内置。
fn resolve_key(user_key: Option<String>) -> Option<String> {
    resolve_key_from(user_key, std::env::var("DASHSCOPE_API_KEY").ok())
}

/// 优先级逻辑本体，env 作为入参传入以便单测（改环境变量在 Rust 2024 是 unsafe，
/// 且会污染并行跑的其他测试）。
fn resolve_key_from(user_key: Option<String>, env_key: Option<String>) -> Option<String> {
    non_blank(user_key)
        .or_else(|| non_blank(env_key))
        .or_else(builtin_key)
}

/// 自然语言 → AI 指令意图（原始 JSON 文本）。
#[tauri::command]
pub async fn ai_generate(
    system_prompt: String,
    user_text: String,
    api_key: Option<String>,
    billing: tauri::State<'_, Billing>,
) -> Result<AiResponse, ()> {
    // 注意：MutexGuard 不能跨 await 持有，这里只取快照。
    let balance_before = billing.balance();
    if balance_before <= 0 {
        return Ok(AiResponse::err("余额不足，请先激活 / 充值。", balance_before));
    }

    let Some(key) = resolve_key(api_key) else {
        return Ok(AiResponse::err(
            "未配置 API key。请在界面上填入通义千问（DashScope）的 API key，或设置环境变量 DASHSCOPE_API_KEY。",
            balance_before,
        ));
    };

    let endpoint = std::env::var("DASHSCOPE_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
    let model = std::env::var("DASHSCOPE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_text },
        ],
        // 强制 JSON 输出，配合前端 parseAiContent
        "response_format": { "type": "json_object" },
        // 指令生成要的是准确而非发散
        "temperature": 0.2,
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Ok(AiResponse::err(format!("创建 HTTP 客户端失败：{e}"), balance_before)),
    };

    let resp = match client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {key}"))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(AiResponse::err(
                format!("网络错误，无法连接大模型接口：{e}"),
                balance_before,
            ))
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let detail: String = resp.text().await.unwrap_or_default().chars().take(300).collect();
        return Ok(AiResponse::err(
            format!("大模型接口返回 {status}：{detail}"),
            balance_before,
        ));
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return Ok(AiResponse::err(format!("解析大模型响应失败：{e}"), balance_before)),
    };

    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|c| !c.trim().is_empty());

    let Some(content) = content else {
        return Ok(AiResponse::err("大模型响应为空。", balance_before));
    };

    let usage = json.get("usage").map(|u| {
        let field = |k: &str| u.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
        AiUsage {
            prompt: field("prompt_tokens"),
            completion: field("completion_tokens"),
            total: field("total_tokens"),
        }
    });

    // 成功才扣费
    let balance = billing.consume();

    Ok(AiResponse {
        ok: true,
        content: Some(content),
        error: None,
        usage,
        balance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_key_wins_over_env_and_is_trimmed() {
        assert_eq!(
            resolve_key_from(Some("  sk-user  ".into()), Some("sk-env".into())),
            Some("sk-user".into()),
        );
    }

    #[test]
    fn falls_back_to_env_when_user_key_blank() {
        // 用户没填（或只敲了空格）时应回落到环境变量，而不是把空串当 key 发出去
        assert_eq!(
            resolve_key_from(Some("   ".into()), Some(" sk-env ".into())),
            Some("sk-env".into()),
        );
        assert_eq!(resolve_key_from(None, Some("sk-env".into())), Some("sk-env".into()));
    }

    #[test]
    fn none_when_nothing_configured() {
        // 内置 key 目前为 None，两处都没有时应明确返回 None，让上层给出可读提示
        assert_eq!(resolve_key_from(None, None), None);
        assert_eq!(resolve_key_from(Some("".into()), Some("  ".into())), None);
    }
}
