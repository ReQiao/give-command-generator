//! AI 桥接：调用任意 OpenAI 兼容的 chat/completions 接口
//! （通义千问 DashScope、OpenAI 官方、或其他兼容服务，由前端传入接口地址 + 模型名）。
//!
//! 分工：
//!   - 系统提示词由前端用 catalog 构造好后传进来，本模块只负责「注入 key + 联网」。
//!   - API key / 接口地址 / 模型名都遵循同一优先级：用户在界面填写 > 环境变量 > 内置默认。
//!     内置默认指向 DashScope，只是为了不配置也能跑；换成 OpenAI 或其他服务只需在界面
//!     填对应的接口地址（含完整路径）、模型名、key 即可，不需要改代码。
//!   - 只把 AI 返回的原始 JSON 文本透传回前端；解析成意图、再确定性地构建命令字符串，
//!     全部由前端的 logic/dispatch.ts 完成——后端不碰命令语法。
//!   - 调用成功才扣 1 次余额（失败不扣）。

use crate::billing::Billing;
use serde::Serialize;

/// 多轮上下文里的一条历史消息（前端只带 user/assistant 两种角色）。
#[derive(serde::Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

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

const DEFAULT_ENDPOINT: &str =
    "https://ws-b2ui8x9tozwc8cq1.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/chat/completions";
const DEFAULT_MODEL: &str = "qwen-plus";

/// 内置 API key（临时明文，仅供本人短期自测用）。
/// ⚠️ 这个 key 已经提交进公开仓库的 git 历史，必须视为已泄露：
/// 用完就去阿里云百炼控制台吊销，不要指望"删掉这行代码"就能撤回。
fn builtin_key() -> Option<String> {
    Some("sk-4c78b2613477463db65ed168beb4af65".to_string())
}

fn non_blank(value: Option<String>) -> Option<String> {
    value.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// 解析出可用的 API key：用户传入 > 环境变量 > 内置。
fn resolve_key(user_key: Option<String>) -> Option<String> {
    resolve_key_from(user_key, std::env::var("AI_API_KEY").ok())
}

/// 优先级逻辑本体，env 作为入参传入以便单测（改环境变量在 Rust 2024 是 unsafe，
/// 且会污染并行跑的其他测试）。
fn resolve_key_from(user_key: Option<String>, env_key: Option<String>) -> Option<String> {
    non_blank(user_key)
        .or_else(|| non_blank(env_key))
        .or_else(builtin_key)
}

/// 解析接口地址 / 模型名：用户传入 > 环境变量 > 内置默认（DashScope）。
/// 逻辑和 key 一致，抽成通用函数方便单测，且不依赖具体 env 变量名。
fn resolve_with_default(user_value: Option<String>, env_value: Option<String>, default: &str) -> String {
    non_blank(user_value)
        .or_else(|| non_blank(env_value))
        .unwrap_or_else(|| default.to_string())
}

/// 自然语言 → AI 指令意图（原始 JSON 文本）。
///
/// `endpoint` / `model` 由前端按用户选择的服务商（DashScope / OpenAI / 自定义）传入，
/// 留空时分别回落到环境变量、再回落到 DashScope 默认值，兼容旧的免配置用法。
#[tauri::command]
pub async fn ai_generate(
    system_prompt: String,
    user_text: String,
    api_key: Option<String>,
    endpoint: Option<String>,
    model: Option<String>,
    // 前端携带的历史轮次（本轮之前的 user/assistant 消息，已经封顶3轮，见 AiPanel）。
    history: Option<Vec<ChatTurn>>,
    billing: tauri::State<'_, Billing>,
) -> Result<AiResponse, ()> {
    // 注意：MutexGuard 不能跨 await 持有，这里只取快照。
    let balance_before = billing.balance();
    if balance_before <= 0 {
        return Ok(AiResponse::err("余额不足，请先激活 / 充值。", balance_before));
    }

    let Some(key) = resolve_key(api_key) else {
        return Ok(AiResponse::err(
            "未配置 API key。请在界面上填入所选服务商的 API key，或设置环境变量 AI_API_KEY。",
            balance_before,
        ));
    };

    let endpoint = resolve_with_default(endpoint, std::env::var("AI_ENDPOINT").ok(), DEFAULT_ENDPOINT);
    let model = resolve_with_default(model, std::env::var("AI_MODEL").ok(), DEFAULT_MODEL);

    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    for turn in history.unwrap_or_default() {
        messages.push(serde_json::json!({ "role": turn.role, "content": turn.content }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": user_text }));

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        // 强制 JSON 输出，配合前端 parseAiContent
        "response_format": { "type": "json_object" },
        // 指令生成要的是准确而非发散
        "temperature": 0.2,
        // qwen3.7 系列（plus/max/flash）是混合推理模型，默认 enable_thinking=true；
        // 思考过程和强制 JSON 输出混在一起，容易导致 JSON 不合法/被截断/意图丢失
        // （表现为"跳过意图"或"瞎编"）。官方文档建议需要稳定 JSON 时关闭思考模式。
        // 不支持这个字段的模型（如 qwen-long、DeepSeek）会直接忽略，不影响调用。
        "enable_thinking": false,
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
    fn falls_back_to_builtin_when_user_and_env_both_blank() {
        // 用户和环境变量都没配置时，应该落到内置 key（临时明文，见 builtin_key 上的警告）
        assert_eq!(resolve_key_from(None, None), builtin_key());
        assert_eq!(resolve_key_from(Some("".into()), Some("  ".into())), builtin_key());
    }

    #[test]
    fn endpoint_and_model_prefer_user_choice_over_env_and_default() {
        // 用户在界面选了 OpenAI，就该打 OpenAI 的地址，不管环境变量或默认值是什么
        assert_eq!(
            resolve_with_default(
                Some("https://api.openai.com/v1/chat/completions".into()),
                Some("https://dashscope.aliyuncs.com/x".into()),
                DEFAULT_ENDPOINT,
            ),
            "https://api.openai.com/v1/chat/completions",
        );
        assert_eq!(resolve_with_default(Some("gpt-4o-mini".into()), None, DEFAULT_MODEL), "gpt-4o-mini");
    }

    #[test]
    fn endpoint_and_model_fall_back_to_env_then_default() {
        assert_eq!(
            resolve_with_default(None, Some("https://example.com/v1/chat/completions".into()), DEFAULT_ENDPOINT),
            "https://example.com/v1/chat/completions",
        );
        assert_eq!(resolve_with_default(Some("  ".into()), None, DEFAULT_MODEL), DEFAULT_MODEL);
        assert_eq!(resolve_with_default(None, None, DEFAULT_ENDPOINT), DEFAULT_ENDPOINT);
    }
}
