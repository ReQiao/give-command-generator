//! `/setblock` 指令构建器。移植自客户端 `src/logic/commands/setblock.ts`。
//!
//! 语法（1.20.5+）：
//!   setblock <x> <y> <z> <block>[blockstate]{nbt} [replace|destroy|keep]
//!
//! 方块实体 NBT 实测真值（semantic-probe 1.20.6 / 1.21.5，两版本一致）：
//!   - 命令方块：{Command:"...", auto:1b, TrackOutput:0b}
//!   - 容器（chest/barrel/hopper）：{Items:[{Slot:0b, id:"...", count:5, components:{...}}]}
//!   - 告示牌：{front_text:{messages:['...',…],color:"black",has_glowing_text:0b}, …}

use crate::give::builder::{bool_byte, quote, GiveVersion};
use crate::give::catalog::namespaced;
use crate::give::commands::nbt::{serialize_container_item, NbtItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetblockMode {
    Replace,
    Destroy,
    Keep,
}

#[derive(Debug, Clone)]
pub struct ContainerSlot {
    pub slot: i64,
    pub item: NbtItem,
}

#[derive(Debug, Clone, Default)]
pub struct SetblockCommandBlockOptions {
    pub command: String,
    /// 始终激活（循环/连锁命令方块常用）。
    pub auto: bool,
    /// 省略视为 true（TS 里 `trackOutput === false` 才输出 0b）。
    pub track_output: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub enum SetblockNbt {
    #[default]
    None,
    CommandBlock(SetblockCommandBlockOptions),
    ContainerItems(Vec<ContainerSlot>),
    SignLines([String; 4]),
}

#[derive(Debug, Clone)]
pub struct SetblockForm {
    pub version: GiveVersion,
    pub with_slash: bool,
    /// 坐标，支持 "0" / "~" / "~1" / "^1" 写法。
    pub x: String,
    pub y: String,
    pub z: String,
    pub block: String,
    /// 方块状态，形如 `facing=up,conditional=false`（不含方括号）。
    pub blockstate: Option<String>,
    pub mode: Option<SetblockMode>,
    /// 方块实体 NBT：三选一，或都留空。
    pub nbt: SetblockNbt,
}

pub fn build_setblock_command(form: &SetblockForm) -> String {
    let block = namespaced(&form.block);
    let blockstate = form.blockstate.as_ref().map(|s| format!("[{s}]")).unwrap_or_default();
    let nbt = build_nbt(form);
    let mode = match form.mode {
        Some(SetblockMode::Destroy) => " destroy",
        Some(SetblockMode::Keep) => " keep",
        Some(SetblockMode::Replace) | None => "",
    };

    // NBT compound 紧贴方块标识符（无空格）：minecraft:command_block[facing=up]{Command:…}
    let cmd = format!("setblock {} {} {} {block}{blockstate}{nbt}{mode}", form.x, form.y, form.z);
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

fn build_nbt(form: &SetblockForm) -> String {
    match &form.nbt {
        SetblockNbt::None => String::new(),
        SetblockNbt::CommandBlock(opts) => build_command_block_nbt(opts),
        SetblockNbt::ContainerItems(items) if !items.is_empty() => build_container_nbt(items),
        SetblockNbt::ContainerItems(_) => String::new(),
        SetblockNbt::SignLines(lines) => build_sign_nbt(lines),
    }
}

fn build_command_block_nbt(opts: &SetblockCommandBlockOptions) -> String {
    // 命令方块内部存的命令不带前导斜杠。
    let trimmed = opts.command.trim();
    let stripped = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let mut parts: Vec<String> = vec![format!("Command:{}", quote(stripped))];
    if opts.auto {
        parts.push("auto:1b".to_string());
    }
    if opts.track_output == Some(false) {
        parts.push("TrackOutput:0b".to_string());
    }
    format!("{{{}}}", parts.join(","))
}

fn build_container_nbt(slots: &[ContainerSlot]) -> String {
    let items = slots
        .iter()
        .map(|s| serialize_container_item(s.slot, &s.item, None))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{Items:[{items}]}}")
}

/// 告示牌 NBT（1.20+ front_text/back_text 格式，两版本一致）。四行定长，空行也要占位。
fn build_sign_nbt(lines: &[String; 4]) -> String {
    let messages = |texts: &[&str]| {
        texts
            .iter()
            .map(|t| quote(&serde_json::json!({ "text": t }).to_string()))
            .collect::<Vec<_>>()
            .join(",")
    };
    let front_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    let front = messages(&front_refs);
    let back = messages(&["", "", "", ""]);
    format!(
        "{{front_text:{{messages:[{front}],color:\"black\",has_glowing_text:{}}},back_text:{{messages:[{back}],color:\"black\",has_glowing_text:{}}},is_waxed:{}}}",
        bool_byte(false),
        bool_byte(false),
        bool_byte(false)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: GiveVersion = GiveVersion::Java1_21_5;

    fn base(block: &str) -> SetblockForm {
        SetblockForm {
            version: MODERN,
            with_slash: false,
            x: "~".to_string(),
            y: "~".to_string(),
            z: "~".to_string(),
            block: block.to_string(),
            blockstate: None,
            mode: None,
            nbt: SetblockNbt::None,
        }
    }

    #[test]
    fn basic() {
        assert_eq!(build_setblock_command(&base("stone")), "setblock ~ ~ ~ minecraft:stone");
    }

    #[test]
    fn blockstate() {
        let mut f = base("oak_log");
        f.blockstate = Some("axis=x".to_string());
        assert_eq!(build_setblock_command(&f), "setblock ~ ~ ~ minecraft:oak_log[axis=x]");
    }

    #[test]
    fn keep_mode() {
        let mut f = base("stone");
        f.mode = Some(SetblockMode::Keep);
        assert_eq!(build_setblock_command(&f), "setblock ~ ~ ~ minecraft:stone keep");
    }

    #[test]
    fn replace_is_default_not_emitted() {
        let mut f = base("stone");
        f.mode = Some(SetblockMode::Replace);
        assert_eq!(build_setblock_command(&f), "setblock ~ ~ ~ minecraft:stone");
    }

    #[test]
    fn command_block_strips_leading_slash() {
        let mut f = base("command_block");
        f.blockstate = Some("facing=up".to_string());
        f.nbt = SetblockNbt::CommandBlock(SetblockCommandBlockOptions {
            command: "/say hi".to_string(),
            auto: true,
            track_output: None,
        });
        assert_eq!(
            build_setblock_command(&f),
            r#"setblock ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi",auto:1b}"#
        );
    }

    #[test]
    fn container_items_slot_upper_count_lower() {
        let mut f = base("chest");
        f.nbt = SetblockNbt::ContainerItems(vec![ContainerSlot {
            slot: 0,
            item: NbtItem { id: "diamond".to_string(), count: Some(5), ..Default::default() },
        }]);
        assert_eq!(
            build_setblock_command(&f),
            r#"setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:5}]}"#
        );
    }

    #[test]
    fn container_items_with_components() {
        let mut f = base("chest");
        f.nbt = SetblockNbt::ContainerItems(vec![ContainerSlot {
            slot: 1,
            item: NbtItem {
                id: "stone".to_string(),
                count: Some(1),
                components: vec![("custom_name".to_string(), r#"'{"text":"x"}'"#.to_string())],
                ..Default::default()
            },
        }]);
        assert_eq!(
            build_setblock_command(&f),
            r#"setblock ~ ~ ~ minecraft:chest{Items:[{Slot:1b,id:"minecraft:stone",count:1,components:{"minecraft:custom_name":'{"text":"x"}'}}]}"#
        );
    }
}
