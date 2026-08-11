//! AI 桥接：把前端的生成请求转发给自建服务器（仓库根目录 server/），本地
//! 不再持有任何真实大模型 key。
//!
//! 这个文件曾经直接持有 API key、价目表、计费逻辑，全部调用真实大模型接口
//! 就在这里完成——直到这个 key 被打进分发的客户端安装包里被人拆出来盗刷了
//! 一次（真实发生的事故）。后来 API key/价目表/扣费判断搬到了服务器
//! （server/src/ai_proxy.rs、server/src/ledger.rs），但"把 AI 意图解析成
//! 指令字符串"这套逻辑（dispatch.ts/builder.ts/commands/*）当时仍然留在
//! 客户端——那才是这个项目真正的护城河（mc-verifier 实测出的语法真值），
//! 比 API key 更值得保护。现在这套逻辑也已经搬到服务器（server/src/give/），
//! 服务器直接返回构建好的指令字符串，这个文件从此也碰不到未经校验的原始
//! AI 输出。
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
    /// 一次性命令，可直接复制/一键部署。
    pub commands: Vec<String>,
    /// 需要持续侦测的命令（execute 意图标了 loop:true 的），单独列出。
    pub loop_commands: Vec<String>,
    /// 构建失败的意图描述，格式 "${command}：${error}"。
    pub failures: Vec<String>,
    /// AI 给出的一句话说明。
    pub explanation: String,
    /// 顶层失败原因（余额不足/上游调用失败/AI 内容解析失败）。
    pub error: Option<String>,
    pub usage: Option<AiUsage>,
    /// 供前端存入多轮对话历史使用，不在 UI 展示。
    pub raw_content: Option<String>,
    /// 当前剩余余额。网络都连不上服务器时是 None——不能瞎填一个 0，
    /// 那会让用户误以为余额真的清零了，而不是网络出了问题。
    pub balance: Option<i64>,
}

/// 自然语言 → 确定性 Minecraft 指令字符串。转发到自建服务器，本地不碰
/// 真实 key、不算价目表、也不再解析/构建指令本体——那些全部在服务器那一侧
/// （见模块顶部注释）。
#[tauri::command]
pub async fn ai_generate(
    system_prompt: String,
    user_text: String,
    // 用户在下拉框选的模型；留空/不传就用服务器 .env 里的默认值。
    model: Option<String>,
    // 目标 Minecraft 版本字符串（如 "java_1_21_11_plus"/"bedrock"），原样
    // 转发给服务器做版本感知的目录校验/指令构建。
    version: String,
    // 前端携带的历史轮次（本轮之前的 user/assistant 消息，已经封顶3轮，见 AiPanel）。
    history: Option<Vec<ChatTurn>>,
) -> Result<AiResponse, ()> {
    let device_id = device::get_or_create();
    let history = history.unwrap_or_default();
    let model = model.as_deref().map(str::trim).filter(|m| !m.is_empty());

    match remote::ai_generate(&device_id, &system_prompt, &user_text, model, &version, &history).await {
        Ok(resp) => Ok(AiResponse {
            ok: resp.ok,
            commands: resp.commands,
            loop_commands: resp.loop_commands,
            failures: resp.failures,
            explanation: resp.explanation,
            error: resp.error,
            usage: resp.usage,
            raw_content: resp.raw_content,
            balance: Some(resp.balance),
        }),
        Err(e) => Ok(AiResponse {
            ok: false,
            commands: Vec::new(),
            loop_commands: Vec::new(),
            failures: Vec::new(),
            explanation: String::new(),
            error: Some(e),
            usage: None,
            raw_content: None,
            balance: None,
        }),
    }
}
