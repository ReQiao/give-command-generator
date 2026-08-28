//! `/enchant` 指令构建器。移植自客户端 `src/logic/commands/enchant.ts`。
//!
//! 语法（1.20.5+ 全版本一致）：enchant <targets> <enchantment> [<level>]
//!
//! 附魔与物品是否兼容、是否超过最高等级由服务器运行时裁决，不属于语法问题，
//! 这里只保证语法正确。

use crate::give::catalog::namespaced;

#[derive(Debug, Clone, Default)]
pub struct EnchantForm {
    pub with_slash: bool,
    pub targets: String,
    pub enchantment: String,
    /// 省略时默认 1。
    pub level: Option<i64>,
}

pub fn build_enchant_command(form: &EnchantForm) -> String {
    let mut cmd = format!("enchant {} {}", form.targets, namespaced(&form.enchantment));
    if let Some(level) = form.level {
        cmd.push(' ');
        cmd.push_str(&level.to_string());
    }
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_level() {
        let form = EnchantForm { targets: "@s".to_string(), enchantment: "sharpness".to_string(), ..Default::default() };
        assert_eq!(build_enchant_command(&form), "enchant @s minecraft:sharpness");
    }

    #[test]
    fn with_level() {
        let form = EnchantForm {
            targets: "@s".to_string(),
            enchantment: "minecraft:unbreaking".to_string(),
            level: Some(3),
            ..Default::default()
        };
        assert_eq!(build_enchant_command(&form), "enchant @s minecraft:unbreaking 3");
    }
}
