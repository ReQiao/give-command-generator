//! `/fill` 指令构建器。移植自客户端 `src/logic/commands/fill.ts`。
//!
//! 语法（1.20.5+ 全版本一致）：
//!   fill <from x y z> <to x y z> <block>[blockstate]{nbt} [destroy|hollow|keep|outline|replace]
//!   fill <from> <to> <block> replace <filterBlock>[filterState]
//!
//! 方块参数与 /setblock 同源：blockstate 用 [..]，方块实体 NBT 用 {..}，两者紧贴方块 id。

use crate::give::catalog::namespaced;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    Replace,
    Destroy,
    Hollow,
    Keep,
    Outline,
}

impl FillMode {
    fn as_str(self) -> &'static str {
        match self {
            FillMode::Replace => "replace",
            FillMode::Destroy => "destroy",
            FillMode::Hollow => "hollow",
            FillMode::Keep => "keep",
            FillMode::Outline => "outline",
        }
    }
}

pub type Coords = [String; 3];

#[derive(Debug, Clone, Default)]
pub struct BlockFilter {
    pub block: String,
    pub blockstate: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FillForm {
    pub with_slash: bool,
    pub from: Coords,
    pub to: Coords,
    pub block: String,
    pub blockstate: Option<String>,
    /// 已序列化的方块实体 NBT 片段，如 `{Command:"say hi"}`。
    pub nbt: Option<String>,
    pub mode: Option<FillMode>,
    /// 指定后强制走 replace 模式，只替换该目标方块。
    pub replace_filter: Option<BlockFilter>,
}

/// 把 block + blockstate + nbt 拼成紧贴的方块参数。
pub fn block_spec(block: &str, blockstate: Option<&str>, nbt: Option<&str>) -> String {
    let state = blockstate.map(|s| format!("[{s}]")).unwrap_or_default();
    let nbt_part = nbt.map(|n| n.trim()).unwrap_or("");
    format!("{}{state}{nbt_part}", namespaced(block))
}

fn join_coords(coords: &Coords) -> String {
    coords.join(" ")
}

pub fn build_fill_command(form: &FillForm) -> String {
    let mut cmd = format!(
        "fill {} {} {}",
        join_coords(&form.from),
        join_coords(&form.to),
        block_spec(&form.block, form.blockstate.as_deref(), form.nbt.as_deref())
    );

    if let Some(filter) = &form.replace_filter {
        cmd.push_str(&format!(" replace {}", block_spec(&filter.block, filter.blockstate.as_deref(), None)));
    } else if let Some(mode) = form.mode {
        if mode != FillMode::Replace {
            cmd.push(' ');
            cmd.push_str(mode.as_str());
        }
    }

    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: &str, y: &str, z: &str) -> Coords {
        [x.to_string(), y.to_string(), z.to_string()]
    }

    #[test]
    fn basic() {
        let form = FillForm { from: c("0", "64", "0"), to: c("10", "70", "10"), block: "stone".to_string(), ..Default::default() };
        assert_eq!(build_fill_command(&form), "fill 0 64 0 10 70 10 minecraft:stone");
    }

    #[test]
    fn hollow_mode() {
        let form = FillForm {
            from: c("~", "~", "~"),
            to: c("~5", "~5", "~5"),
            block: "glass".to_string(),
            mode: Some(FillMode::Hollow),
            ..Default::default()
        };
        assert_eq!(build_fill_command(&form), "fill ~ ~ ~ ~5 ~5 ~5 minecraft:glass hollow");
    }

    #[test]
    fn replace_filter_block() {
        let form = FillForm {
            from: c("0", "64", "0"),
            to: c("10", "70", "10"),
            block: "air".to_string(),
            replace_filter: Some(BlockFilter { block: "water".to_string(), blockstate: None }),
            ..Default::default()
        };
        assert_eq!(build_fill_command(&form), "fill 0 64 0 10 70 10 minecraft:air replace minecraft:water");
    }

    #[test]
    fn blockstate_and_nbt_touching_block_id() {
        let form = FillForm {
            from: c("~", "~", "~"),
            to: c("~", "~", "~"),
            block: "command_block".to_string(),
            blockstate: Some("facing=up".to_string()),
            nbt: Some(r#"{Command:"say hi"}"#.to_string()),
            ..Default::default()
        };
        assert_eq!(
            build_fill_command(&form),
            r#"fill ~ ~ ~ ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi"}"#
        );
    }
}
