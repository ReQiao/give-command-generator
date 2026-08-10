//! AI 桥接：把前端的生成请求转发给自建服务器（仓库根目录 server/），本地
//! 不再持有任何真实大模型 key。
//!
//! 这个文件曾经直接持有 API key、价目表、计费逻辑，全部调用真实大模型接口
//! 就在这里完成——直到这个 key 被打进分发的客户端安装包里被人拆出来盗刷了
//! 一次（真实发生的事故）。现在这个文件唯一的职责是：把前端传来的
//! system_prompt / user_text / history 转发给 remote::ai_generate，原样
//! 透传结果。真实 key、价目表、真正的扣费判断全部搬到了服务器
//! （server/src/ai_proxy.rs、server/src/ledger.rs），本地这份代码从此
//! 碰不到能直接花钱的凭证。

use crate::device;
use crate::remote;
use serde::{Deserialize, Serialize};

/// 多轮上下文里的一条历史消息（前端只带 user/assistant 两种角色）。
/// 既要能从前端反序列化进来，也要能原样序列化转发给服务器，所以两个都要。
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResponse {
    pub ok: bool,
    /// AI 返回的原始 JSON 文本（成功时）。
    pub content: Option<String>,
    /// 失败原因（失败时）。
    pub error: Option<String>,
    pub usage: Option<AiUsage>,
    /// 当前剩余余额。网络都连不上服务器时是 None——不能瞎填一个 0，
    /// 那会让用户误以为余额真的清零了，而不是网络出了问题。
    pub balance: Option<i64>,
}

/// 自然语言 → AI 指令意图（原始 JSON 文本）。转发到自建服务器，本地不碰
/// 真实 key、不算价目表——那些全部在服务器那一侧（见模块顶部注释）。
#[tauri::command]
pub async fn ai_generate(
    system_prompt: String,
    user_text: String,
    // 前端携带的历史轮次（本轮之前的 user/assistant 消息，已经封顶3轮，见 AiPanel）。
    history: Option<Vec<ChatTurn>>,
) -> Result<AiResponse, ()> {
    let device_id = device::get_or_create();
    let history = history.unwrap_or_default();

    match remote::ai_generate(&device_id, &system_prompt, &user_text, &history).await {
        Ok(resp) => Ok(AiResponse {
            ok: resp.ok,
            content: resp.content,
            error: resp.error,
            usage: resp.usage,
            balance: Some(resp.balance),
        }),
        Err(e) => Ok(AiResponse { ok: false, content: None, error: Some(e), usage: None, balance: None }),
    }
}
