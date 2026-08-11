//! `/clone` 指令构建器。移植自客户端 `src/logic/commands/clone.ts`。
//!
//! 语法（1.20.5+ 全版本一致，含 1.19.4+ 引入的跨维度形式）：
//!   clone [from <dim>] <begin> <end> [to <dim>] <dest> [replace|masked|filtered <filter>] [normal|force|move]
//!
//! 注意：/clone 不支持本地坐标（^），只能用绝对或相对坐标。

use crate::give::catalog::namespaced;
use crate::give::commands::fill::{block_spec, BlockFilter, Coords};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneMaskMode {
    Replace,
    Masked,
    Filtered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneMode {
    Normal,
    Force,
    Move,
}

#[derive(Debug, Clone, Default)]
pub struct CloneForm {
    pub with_slash: bool,
    /// 源维度，如 "minecraft:the_nether"。省略则用执行者所在维度。
    pub from_dimension: Option<String>,
    pub begin: Coords,
    pub end: Coords,
    /// 目标维度。省略则与源维度一致。
    pub to_dimension: Option<String>,
    pub destination: Coords,
    pub mask_mode: Option<CloneMaskMode>,
    /// maskMode === "filtered" 时必填。
    pub filter: Option<BlockFilter>,
    pub clone_mode: Option<CloneMode>,
}

pub fn build_clone_command(form: &CloneForm) -> Result<String, String> {
    let mut parts: Vec<String> = vec!["clone".to_string()];

    if let Some(dim) = &form.from_dimension {
        parts.push("from".to_string());
        parts.push(namespaced(dim));
    }
    parts.push(form.begin.join(" "));
    parts.push(form.end.join(" "));
    if let Some(dim) = &form.to_dimension {
        parts.push("to".to_string());
        parts.push(namespaced(dim));
    }
    parts.push(form.destination.join(" "));

    match form.mask_mode {
        Some(CloneMaskMode::Filtered) => {
            let filter = form
                .filter
                .as_ref()
                .ok_or_else(|| "clone filtered 模式需要提供 filter 过滤方块。".to_string())?;
            parts.push("filtered".to_string());
            parts.push(block_spec(&filter.block, filter.blockstate.as_deref(), None));
        }
        Some(CloneMaskMode::Masked) => parts.push("masked".to_string()),
        Some(CloneMaskMode::Replace) | None => {}
    }

    if let Some(clone_mode) = form.clone_mode {
        if clone_mode != CloneMode::Normal {
            // cloneMode 必须跟在 maskMode 之后；未指定 maskMode 时补默认 replace。
            if form.mask_mode.is_none() {
                parts.push("replace".to_string());
            }
            parts.push(
                match clone_mode {
                    CloneMode::Force => "force",
                    CloneMode::Move => "move",
                    CloneMode::Normal => unreachable!(),
                }
                .to_string(),
            );
        }
    }

    let cmd = parts.join(" ");
    Ok(if form.with_slash { format!("/{cmd}") } else { cmd })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: &str, y: &str, z: &str) -> Coords {
        [x.to_string(), y.to_string(), z.to_string()]
    }

    #[test]
    fn basic() {
        let form = CloneForm {
            begin: c("0", "64", "0"),
            end: c("10", "70", "10"),
            destination: c("20", "64", "20"),
            ..Default::default()
        };
        assert_eq!(build_clone_command(&form).unwrap(), "clone 0 64 0 10 70 10 20 64 20");
    }

    #[test]
    fn cross_dimension() {
        let form = CloneForm {
            from_dimension: Some("the_nether".to_string()),
            begin: c("0", "64", "0"),
            end: c("10", "70", "10"),
            to_dimension: Some("overworld".to_string()),
            destination: c("20", "64", "20"),
            ..Default::default()
        };
        assert_eq!(
            build_clone_command(&form).unwrap(),
            "clone from minecraft:the_nether 0 64 0 10 70 10 to minecraft:overworld 20 64 20"
        );
    }

    #[test]
    fn filtered_with_filter_block() {
        let form = CloneForm {
            begin: c("0", "64", "0"),
            end: c("10", "70", "10"),
            destination: c("20", "64", "20"),
            mask_mode: Some(CloneMaskMode::Filtered),
            filter: Some(BlockFilter { block: "stone".to_string(), blockstate: None }),
            ..Default::default()
        };
        assert_eq!(
            build_clone_command(&form).unwrap(),
            "clone 0 64 0 10 70 10 20 64 20 filtered minecraft:stone"
        );
    }

    #[test]
    fn move_mode_defaults_replace() {
        let form = CloneForm {
            begin: c("0", "64", "0"),
            end: c("10", "70", "10"),
            destination: c("20", "64", "20"),
            clone_mode: Some(CloneMode::Move),
            ..Default::default()
        };
        assert_eq!(build_clone_command(&form).unwrap(), "clone 0 64 0 10 70 10 20 64 20 replace move");
    }

    #[test]
    fn filtered_without_filter_errors() {
        let form = CloneForm {
            begin: c("0", "64", "0"),
            end: c("10", "70", "10"),
            destination: c("20", "64", "20"),
            mask_mode: Some(CloneMaskMode::Filtered),
            ..Default::default()
        };
        assert!(build_clone_command(&form).is_err());
    }
}
