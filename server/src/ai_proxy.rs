//! AI 调用代理：所有客户端请求打到这里，由服务器用自己配置的真实大模型 key
//! 转发给上游接口。key 从此只活在这台服务器进程的环境变量里，不再跟着客户端
//! 分发——这是这整个服务器要解决的核心问题（见 main.rs 顶部注释）。
//!
//! 价目表 / 计费逻辑照搬自 src-tauri/src/ai.rs（那份此后应该只保留客户端展示，
//! 真正计费判断以服务器这份为准）。

use serde::{Deserialize, Serialize};

/// 多轮上下文里的一条历史消息（客户端只带 user/assistant 两种角色）。
#[derive(Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiUsage {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

/// 服务器自己的大模型配置：接口地址 / 模型名 / key，全部从环境变量读取，
/// 不接受客户端传入——这正是这次改造的意义所在，客户端不再有资格指定
/// "用哪个 key 打哪个接口"，那本身就是泄露的根源。
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
    pub key: String,
}

impl AiConfig {
    /// 三个环境变量缺一不可，缺了直接让服务起不来，而不是悄悄拿空字符串去
    /// 请求上游——那样每次调用都会失败，而且不容易第一时间发现是配置问题
    /// 还是别的故障。
    pub fn from_env() -> Self {
        let get = |name: &str| {
            std::env::var(name).unwrap_or_else(|_| panic!("缺少环境变量 {name}，服务无法启动"))
        };
        AiConfig { endpoint: get("AI_ENDPOINT"), model: get("AI_MODEL"), key: get("AI_API_KEY") }
    }
}

/// 1 元人民币兑换的灵魂币消耗倍率，和客户端 ai.rs 的约定完全一致。
const COIN_PER_YUAN_SPENT: f64 = 1500.0;

struct ModelPrice {
    input_per_million: f64,
    output_per_million: f64,
}

/// 价目表照搬自客户端 ai.rs::model_price，两边曾经保持一致，现在客户端那份
/// 应该退役（不再实际参与计费，只有服务器这份说了算）。
fn model_price(model: &str, input_tokens: u32) -> ModelPrice {
    match model {
        "qwen3.8-max" => ModelPrice { input_per_million: 12.0, output_per_million: 36.0 },
        "qwen3.7-plus" | "qwen-plus" => ModelPrice { input_per_million: 2.0, output_per_million: 8.0 },
        "qwen3.7-flash" => {
            if input_tokens > 32_000 {
                ModelPrice { input_per_million: 0.6, output_per_million: 2.4 }
            } else {
                ModelPrice { input_per_million: 0.2, output_per_million: 0.8 }
            }
        }
        "qwen-long-latest" | "qwen-long" => ModelPrice { input_per_million: 0.5, output_per_million: 2.0 },
        "deepseek-v4-pro" => ModelPrice { input_per_million: 12.0, output_per_million: 24.0 },
        _ => ModelPrice { input_per_million: 2.0, output_per_million: 8.0 },
    }
}

/// 按真实 token 用量折算这次调用该扣多少灵魂币，逻辑和客户端 ai.rs::coins_to_charge
/// 一致（包括拿不到 usage 时的保守兜底、向上取整）。
pub fn coins_to_charge(model: &str, usage: Option<&AiUsage>) -> i64 {
    const FALLBACK_COINS: i64 = 100;
    let Some(usage) = usage else { return FALLBACK_COINS };
    let price = model_price(model, usage.prompt);
    let yuan = (usage.prompt as f64 / 1_000_000.0) * price.input_per_million
        + (usage.completion as f64 / 1_000_000.0) * price.output_per_million;
    ((yuan * COIN_PER_YUAN_SPENT).ceil() as i64).max(1)
}

/// 真正打上游大模型接口。失败原因直接透传给客户端展示（同客户端 ai.rs 的做法），
/// 里面不含任何密钥信息，可以放心展示给用户。
///
/// `model` 由调用方决定用哪一个——不再固定死用 `config.model`，因为客户端
/// 恢复了模型下拉可选（Plus/Max/Flash/Long/DeepSeek 价格/能力差很多，用户
/// 想自己权衡）。`config` 里的 endpoint/key 依然是服务器唯一权威来源，
/// 不接受客户端指定——那两个是真正跟钱/身份挂钩的东西，模型名不是。
pub async fn call_upstream(
    config: &AiConfig,
    model: &str,
    system_prompt: &str,
    user_text: &str,
    history: Vec<ChatTurn>,
) -> Result<(String, Option<AiUsage>), String> {
    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    for turn in history {
        messages.push(serde_json::json!({ "role": turn.role, "content": turn.content }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": user_text }));

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "response_format": { "type": "json_object" },
        "temperature": 0.2,
        // qwen3.7 系列默认开思考模式，跟强制 JSON 输出混在一起容易导致 JSON
        // 不合法/被截断，关掉更稳；不支持这个字段的模型会直接忽略。
        "enable_thinking": false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))?;

    let resp = client
        .post(&config.endpoint)
        .header("Authorization", format!("Bearer {}", config.key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误，无法连接大模型接口：{e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let detail: String = resp.text().await.unwrap_or_default().chars().take(300).collect();
        return Err(format!("大模型接口返回 {status}：{detail}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("解析大模型响应失败：{e}"))?;

    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|c| !c.trim().is_empty());

    let Some(content) = content else {
        return Err("大模型响应为空。".to_string());
    };

    let usage = json.get("usage").map(|u| {
        let field = |k: &str| u.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
        AiUsage {
            prompt: field("prompt_tokens"),
            completion: field("completion_tokens"),
            total: field("total_tokens"),
        }
    });

    Ok((content, usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 计费逻辑的测试直接照搬自客户端 ai.rs 那五条——两边算法必须保持一致，
    // 独立各写一遍是为了在两个不同的二进制里都能跑，不依赖对方存在。

    #[test]
    fn coins_to_charge_matches_hand_calculated_qwen_plus_example() {
        let usage = AiUsage { prompt: 19_000, completion: 600, total: 19_600 };
        let coins = coins_to_charge("qwen3.7-plus", Some(&usage));
        assert!((60..=75).contains(&coins), "算出来是 {coins}，应该落在 60~75 附近");
    }

    #[test]
    fn coins_to_charge_flash_jumps_tier_past_32k_input() {
        let under = AiUsage { prompt: 31_000, completion: 600, total: 31_600 };
        let over = AiUsage { prompt: 33_000, completion: 600, total: 33_600 };
        let coins_under = coins_to_charge("qwen3.7-flash", Some(&under));
        let coins_over = coins_to_charge("qwen3.7-flash", Some(&over));
        assert!(coins_over > coins_under);
        assert!(coins_over as f64 / coins_under as f64 > 2.5);
    }

    #[test]
    fn coins_to_charge_falls_back_when_usage_missing() {
        assert_eq!(coins_to_charge("qwen3.7-plus", None), 100);
    }

    #[test]
    fn coins_to_charge_never_zero_even_for_tiny_usage() {
        let usage = AiUsage { prompt: 1, completion: 0, total: 1 };
        assert!(coins_to_charge("qwen-long-latest", Some(&usage)) >= 1);
    }

    // AiConfig::from_env 的"缺环境变量就 panic"行为没有配套单测：改 std::env
    // 在 Rust 2024 是 unsafe 操作，而且会污染同进程里并行跑的其它测试
    // （这条早前在 src-tauri/src/ai.rs 的测试注释里就提过，这里不重复踩坑）。
    // 逻辑本身只是一行 unwrap_or_else(panic!)，靠代码审查确认就够了。
}
