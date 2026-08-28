//! 解析 AI 返回的原始 JSON 文本为指令意图。移植自客户端
//! `src/logic/ai/prompt.ts::parseAiContent`（连同 `stripCodeFence`、
//! `KNOWN_COMMANDS`）。
//!
//! `buildSystemPrompt`（提示词构建）不在这次迁移范围——它是喂给大模型的
//! 自然语言文本，不是确定性逻辑，没有安全/正确性问题，继续留在客户端。

use serde_json::Value;

use crate::give::dispatch::CommandIntent;

pub struct ParsedAi {
    pub intents: Vec<CommandIntent>,
    pub explanation: String,
}

/// 支持的意图 command 取值，用于过滤模型偶尔混入 intents 数组的非法项。
const KNOWN_COMMANDS: &[&str] = &[
    "give",
    "say",
    "effect_give",
    "effect_clear",
    "tp",
    "setblock",
    "summon",
    "fill",
    "clone",
    "enchant",
    "execute",
    "scoreboard",
    "attribute",
    "particle",
];

/// 模型偶尔会把 JSON 包在 ```json 代码块里，宽容处理一下。
/// 对应 TS 正则 `^```(?:json)?\s*\n([\s\S]*?)\n?```$`。
fn strip_code_fence(content: &str) -> String {
    let text = content.trim();
    let Some(rest) = text.strip_prefix("```") else {
        return text.to_string();
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    // 代码块围栏后必须紧跟换行（允许围栏和换行之间有空白），否则不算命中。
    let after_fence_marker = rest.trim_start_matches([' ', '\t']);
    let Some(body) = after_fence_marker.strip_prefix('\n') else {
        return text.to_string();
    };
    let Some(body) = body.strip_suffix("```") else {
        return text.to_string();
    };
    body.trim_end_matches('\n').trim().to_string()
}

/// 解析 AI 返回的 JSON 文本为指令意图。
pub fn parse_ai_content(content: &str) -> Result<ParsedAi, String> {
    let text = strip_code_fence(content);
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| format!("无法解析 AI 返回的 JSON：{}", truncate_chars(&text, 200)))?;

    let Some(raw_intents) = parsed.get("intents").and_then(Value::as_array) else {
        return Err("AI 返回缺少 intents 数组。".to_string());
    };

    // 模型偶尔会把 explanation 错放进 intents 数组里（形如 {"explanation":"..."}，
    // 没有 command 字段），这不是一条合法指令——之前会被 dispatch 当成"未知指令类型"
    // 报错，实际上应该静默兜底：捞出来当 explanation 用，而不是当成失败项展示给用户。
    let mut explanation = parsed
        .get("explanation")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut intents = Vec::new();
    for item in raw_intents {
        let Some(obj) = item.as_object() else { continue };
        let command = obj.get("command").and_then(Value::as_str);
        if let Some(command) = command.filter(|c| KNOWN_COMMANDS.contains(c)) {
            match obj.get("form") {
                Some(form) if form.is_object() => {
                    intents.push(CommandIntent::new_public(command, form.clone()));
                }
                _ => {
                    // 模型偶尔会忘记套 form 包装，把参数直接摊平写在意图对象上
                    // （例如 { "command": "execute", "subcommands": [...], "run": "..." }，
                    // 没有 form 字段）。这会导致构建器拿到空表单——execute 报"至少需要
                    // 一个子命令"、give 直接退回默认物品——看起来像是随机丢参数，其实是
                    // 同一个结构性问题。这里兜底：把除 command 外的其余字段收拢成 form。
                    let rest: serde_json::Map<String, Value> = obj
                        .iter()
                        .filter(|(k, _)| k.as_str() != "command")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    intents.push(CommandIntent::new_public(command, Value::Object(rest)));
                }
            }
            continue;
        }
        if explanation.is_empty() {
            if let Some(e) = obj.get("explanation").and_then(Value::as_str) {
                explanation = e.to_string();
            }
        }
    }

    Ok(ParsedAi { intents, explanation })
}

/// 按字符（不是字节）截断，避免在多字节 UTF-8 字符中间切断导致 panic——
/// TS 的 `slice(0, 200)` 是按 UTF-16 code unit 截断，这里按 Unicode 标量值
/// 截断已经足够贴近实际用途（这段文本只用于报错信息展示）。
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::give::builder::GiveVersion;
    use crate::give::dispatch::dispatch_intents;

    const MODERN: GiveVersion = GiveVersion::Java1_21_5;

    #[test]
    fn parses_intents_and_explanation() {
        let r = parse_ai_content(
            r#"{"intents":[{"command":"say","form":{"message":"hi"}}],"explanation":"打个招呼"}"#,
        )
        .unwrap();
        assert_eq!(r.intents.len(), 1);
        assert_eq!(r.explanation, "打个招呼");
    }

    #[test]
    fn tolerates_json_code_fence() {
        let r = parse_ai_content("```json\n{\"intents\":[],\"explanation\":\"x\"}\n```").unwrap();
        assert_eq!(r.explanation, "x");
    }

    #[test]
    fn explanation_misplaced_in_intents_array_is_filtered_and_recovered() {
        let r = parse_ai_content(
            r#"{"intents":[{"command":"say","form":{"message":"hi"}},{"explanation":"这是解释"}],"explanation":""}"#,
        )
        .unwrap();
        assert_eq!(r.intents.len(), 1);
        assert_eq!(r.explanation, "这是解释");
    }

    #[test]
    fn top_level_explanation_takes_priority() {
        let r = parse_ai_content(
            r#"{"intents":[{"explanation":"数组里的"}],"explanation":"顶层的"}"#,
        )
        .unwrap();
        assert_eq!(r.explanation, "顶层的");
        assert_eq!(r.intents.len(), 0);
    }

    #[test]
    fn missing_form_wrapper_is_recovered_and_dispatches_correctly() {
        let payload = serde_json::json!({
            "intents": [
                { "command": "give", "item": "minecraft:bow", "count": 1 },
                { "command": "execute", "subcommands": ["at @e[type=minecraft:arrow]"], "run": "kill @s", "loop": true },
            ],
            "explanation": "x",
        });
        let r = parse_ai_content(&payload.to_string()).unwrap();
        assert_eq!(r.intents.len(), 2);
        assert_eq!(r.intents[0].form().get("item").and_then(Value::as_str), Some("minecraft:bow"));
        assert_eq!(
            r.intents[1].form().get("subcommands").and_then(Value::as_array).map(|a| a.len()),
            Some(1)
        );
        assert_eq!(r.intents[1].form().get("loop").and_then(Value::as_bool), Some(true));

        let results = dispatch_intents(r.intents, MODERN);
        assert_eq!(results[0].command.as_deref(), Some("give @a minecraft:bow 1"));
        assert_eq!(
            results[1].command.as_deref(),
            Some("execute at @e[type=minecraft:arrow] run kill @s")
        );
    }

    #[test]
    fn existing_form_wrapper_is_preserved_untouched() {
        let r = parse_ai_content(
            &serde_json::json!({ "intents": [{ "command": "say", "form": { "message": "hi" } }], "explanation": "x" })
                .to_string(),
        )
        .unwrap();
        assert_eq!(r.intents[0].form().get("message").and_then(Value::as_str), Some("hi"));
    }

    #[test]
    fn non_json_is_an_error() {
        assert!(parse_ai_content("对不起，我做不到").is_err());
    }

    #[test]
    fn missing_intents_array_is_an_error() {
        assert!(parse_ai_content(r#"{"explanation":"x"}"#).is_err());
    }

    // ---------------- 端到端：AI 响应 → 命令字符串 ----------------

    #[test]
    fn end_to_end_explosive_arrow_and_landmine() {
        let payload = serde_json::json!({
            "intents": [
                { "command": "give", "form": { "item": "minecraft:bow", "count": 1, "enchantments": [{ "id": "minecraft:power", "level": 5 }] } },
                {
                    "command": "execute",
                    "form": {
                        "subcommands": ["at @e[type=minecraft:arrow,nbt={inGround:1b}]"],
                        "run": "summon minecraft:tnt ~ ~ ~ {fuse:0s}",
                    },
                },
            ],
            "explanation": "把第二条放进循环命令方块",
        });
        let ai = parse_ai_content(&payload.to_string()).unwrap();
        let results = dispatch_intents(ai.intents, MODERN);
        assert_eq!(results[0].command.as_deref(), Some("give @a minecraft:bow[enchantments={power:5}] 1"));
        assert_eq!(
            results[1].command.as_deref(),
            Some("execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}")
        );
    }

    #[test]
    fn end_to_end_custom_data_arrow() {
        let payload = serde_json::json!({
            "intents": [
                { "command": "give", "form": { "item": "minecraft:bow", "count": 1 } },
                { "command": "give", "form": { "item": "minecraft:arrow", "count": 16, "customData": "{soul_tnt_arrow:1b}" } },
                {
                    "command": "execute",
                    "form": {
                        "subcommands": ["at @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}]"],
                        "run": "summon minecraft:tnt ~ ~ ~ {fuse:0s}",
                    },
                },
            ],
            "explanation": "特制箭打了标记，只有这种箭落地才炸",
        });
        let ai = parse_ai_content(&payload.to_string()).unwrap();
        let results = dispatch_intents(ai.intents, MODERN);
        assert_eq!(results[0].command.as_deref(), Some("give @a minecraft:bow 1"));
        assert_eq!(results[1].command.as_deref(), Some("give @a minecraft:arrow[custom_data={soul_tnt_arrow:1b}] 16"));
        assert_eq!(
            results[2].command.as_deref(),
            Some("execute at @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}")
        );
    }

    #[test]
    fn end_to_end_zombie_boss_with_health_and_equipment() {
        let payload = serde_json::json!({
            "intents": [
                {
                    "command": "summon",
                    "form": {
                        "entityType": "minecraft:zombie",
                        "noAI": true,
                        "health": 40,
                        "attributes": [{ "id": "max_health", "base": 40 }],
                        "equipment": {
                            "mainhand": { "id": "minecraft:diamond_sword", "enchantments": [{ "id": "minecraft:sharpness", "level": 5 }] },
                        },
                    },
                },
            ],
            "explanation": "40 血用属性+当前值同时设置，剑的附魔走结构化字段",
        });
        let ai = parse_ai_content(&payload.to_string()).unwrap();
        let results = dispatch_intents(ai.intents, MODERN);
        assert_eq!(
            results[0].command.as_deref(),
            Some(r#"summon minecraft:zombie ~ ~ ~ {NoAI:1b,Health:40f,attributes:[{id:"minecraft:max_health",base:40d}],equipment:{mainhand:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{sharpness:5}}}}}"#)
        );
    }
}
