//! AI 桥接 —— **录视频分支专用版本，和 main 上的不是一回事**。
//!
//! # 这个分支为什么和主线不同
//!
//! 主线上这个文件只是个转发层：请求打到自建服务器，key、价目表、扣费判断、
//! 以及"把 AI 意图变成确定性指令"那套逻辑全在服务器那一侧
//! （server/src/ai_proxy.rs、server/src/give/）。那样做有两个原因，都别忘了：
//!
//!   1. 这个文件曾经直接持有 API key，key 被打进分发的安装包里、被人拆出来
//!      盗刷过一次（真实发生的事故）。
//!   2. "AI 意图 → 合法指令"这套东西是 mc-verifier 实测出来的语法真值，
//!      是这个项目真正的护城河，比 key 更值得保护。
//!
//! 这个分支把两样都搬回了客户端，是为了录演示视频：不用起服务端、不用配环境
//! 变量、断网也能跑。**它只服务于录制，不要合进 main。**
//!
//! # 但和出事故的那一版有一个关键区别
//!
//! 那次事故的根源是**软件里内置了作者的 key**，所有人共用一把、拆出来就能白嫖。
//! 这里没有任何内置 key：`resolve_key` 只认用户自己在界面上填的那一个，
//! 填的是谁的 key 就花谁的钱。这条线不能松——一旦给它加个"默认 key"兜底，
//! 就是原样重演当年那次事故。
use serde::{Deserialize, Serialize};

/// 留空时的回落值。只是省得用户手打，不涉及任何凭据。
const DEFAULT_ENDPOINT: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
const DEFAULT_MODEL: &str = "qwen-plus";

/// 多轮上下文里的一条历史消息（前端只带 user/assistant 两种角色）。
#[derive(Serialize, Deserialize, Clone)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiUsage {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

/// 只回原始文本，不回指令。
///
/// 解析和构建放在前端（src/logic/ai/prompt.ts 的 parseAiContent →
/// src/logic/dispatch.ts → src/logic/commands/*），因为那套构建器是 TypeScript
/// 写的，本来就住在前端；主线上是服务器用 Rust 重写了一份。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResponse {
    pub ok: bool,
    /// AI 返回的原始 JSON 文本（成功时）。
    pub content: Option<String>,
    /// 失败原因（失败时）。
    pub error: Option<String>,
    pub usage: Option<AiUsage>,
}

impl AiResponse {
    fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, content: None, error: Some(msg.into()), usage: None }
    }
}

/// 用户填了就用用户的，否则没有——**不设内置兜底**（理由见模块顶部注释）。
fn resolve_key(user_key: Option<String>) -> Option<String> {
    user_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// endpoint / model 允许留空回落到默认值：它们不是凭据，写死一个常见值只是省事。
fn resolve_with_default(user_value: Option<String>, default: &str) -> String {
    user_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// 自然语言 → AI 指令意图（原始 JSON 文本）。直连用户自己填的 OpenAI 兼容端点。
#[tauri::command]
pub async fn ai_generate(
    system_prompt: String,
    user_text: String,
    api_key: Option<String>,
    endpoint: Option<String>,
    model: Option<String>,
    history: Option<Vec<ChatTurn>>,
) -> Result<AiResponse, ()> {
    let Some(key) = resolve_key(api_key) else {
        return Ok(AiResponse::err("还没填 API key。在上面的「AI 服务商」里填入你自己的 key。"));
    };
    let endpoint = resolve_with_default(endpoint, DEFAULT_ENDPOINT);
    let model = resolve_with_default(model, DEFAULT_MODEL);

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
        // qwen3 系列是混合推理模型，默认 enable_thinking=true；思考过程和强制
        // JSON 输出混在一起，容易导致 JSON 不合法/被截断/意图丢失（表现为
        // "跳过意图"或"瞎编"）。不支持这个字段的模型会直接忽略。
        "enable_thinking": false,
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Ok(AiResponse::err(format!("创建 HTTP 客户端失败：{e}"))),
    };

    let resp = match client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {key}"))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Ok(AiResponse::err(format!("网络错误，无法连接大模型接口：{e}"))),
    };

    let status = resp.status();
    if !status.is_success() {
        // 只截前 300 字：上游的错误体有时会把整个请求回显出来，那里面有 key。
        let detail: String = resp.text().await.unwrap_or_default().chars().take(300).collect();
        return Ok(AiResponse::err(format!("大模型接口返回 {status}：{detail}")));
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return Ok(AiResponse::err(format!("解析大模型响应失败：{e}"))),
    };

    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|c| !c.trim().is_empty());

    let Some(content) = content else {
        return Ok(AiResponse::err("大模型响应为空。"));
    };

    let usage = json.get("usage").map(|u| {
        let field = |k: &str| u.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        AiUsage {
            prompt: field("prompt_tokens"),
            completion: field("completion_tokens"),
            total: field("total_tokens"),
        }
    });

    Ok(AiResponse { ok: true, content: Some(content), error: None, usage })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 这条测试守的是模块顶部那条线：**没有内置 key**。
    /// 哪怕以后有人图方便想加个默认值兜底，这里会先红。
    #[test]
    fn no_built_in_key_fallback() {
        assert_eq!(resolve_key(None), None);
        assert_eq!(resolve_key(Some("   ".into())), None);
        assert_eq!(resolve_key(Some("  sk-abc  ".into())), Some("sk-abc".into()));
    }

    #[test]
    fn endpoint_and_model_fall_back_to_defaults() {
        assert_eq!(resolve_with_default(None, DEFAULT_MODEL), DEFAULT_MODEL);
        assert_eq!(resolve_with_default(Some("  ".into()), DEFAULT_MODEL), DEFAULT_MODEL);
        assert_eq!(resolve_with_default(Some("gpt-4o-mini".into()), DEFAULT_MODEL), "gpt-4o-mini");
    }
}
