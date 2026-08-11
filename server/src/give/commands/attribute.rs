//! `/attribute` 指令构建器。移植自客户端 `src/logic/commands/attribute.ts`。
//!
//! 这是唯一需要两条独立版本边界的指令：
//!
//! 【边界 A：属性 id 前缀】（与实体 NBT 同一注册表，semantic-probe 1.20.6/1.21.5 实证）
//!   - 1.20.5 ~ 1.21.4：带类别前缀，如 minecraft:generic.max_health
//!   - 1.21.5+：去前缀，如 minecraft:max_health
//!   复用 is_modern_nbt_family 判定，与 nbt.rs 的属性序列化边界保持一致。
//!
//! 【边界 B：modifier 子命令格式】（1.21 属性系统重写）
//!   - 1.20.5/1.20.6：modifier add <uuid> <name> <value> <operation>
//!       operation ∈ {add, multiply_base, multiply}
//!   - 1.21+：modifier add <id> <value> <operation>（无独立 name）
//!       operation ∈ {add_value, add_multiplied_base, add_multiplied_total}
//!
//! 其余子命令跨版本一致：
//!   attribute <target> <attribute> get|base get [<scale>] / base set <value>
//!   attribute <target> <attribute> modifier remove|value get <id> [<scale>]

use crate::give::builder::{fmt_number_f64, is_java1205_family, is_modern_nbt_family, GiveVersion};
use crate::give::catalog::namespaced;
use crate::give::commands::nbt::normalize_attribute_id;

/// 归一化的运算类型（与版本无关），输出时按版本映射到具体关键字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperation {
    Add,
    MultiplyBase,
    MultiplyTotal,
}

#[derive(Debug, Clone)]
pub enum AttributeAction {
    Get { scale: Option<f64> },
    BaseGet { scale: Option<f64> },
    BaseSet { value: f64 },
    ModifierAdd {
        /// 1.21+ 为资源 id（如 "minecraft:my_buff"）；1.20.5/1.20.6 为 UUID 字符串。
        id: String,
        /// 仅 1.20.5/1.20.6 需要；1.21+ 忽略。
        name: Option<String>,
        value: f64,
        operation: AttributeOperation,
    },
    ModifierRemove { id: String },
    ModifierValueGet { id: String, scale: Option<f64> },
}

#[derive(Debug, Clone)]
pub struct AttributeForm {
    pub version: GiveVersion,
    pub with_slash: bool,
    pub target: String,
    /// 属性 id，可带或不带 generic. / minecraft: 前缀，builder 按版本归一化。
    pub attribute: String,
    pub action: AttributeAction,
}

/// 按版本映射运算关键字（边界 B）。
fn operation_keyword(op: AttributeOperation, legacy: bool) -> &'static str {
    if legacy {
        return match op {
            AttributeOperation::MultiplyTotal => "multiply",
            AttributeOperation::Add => "add",
            AttributeOperation::MultiplyBase => "multiply_base",
        };
    }
    match op {
        AttributeOperation::Add => "add_value",
        AttributeOperation::MultiplyBase => "add_multiplied_base",
        AttributeOperation::MultiplyTotal => "add_multiplied_total",
    }
}

pub fn build_attribute_command(form: &AttributeForm) -> String {
    // 1.20.5/1.20.6 使用旧版 modifier 格式（UUID + name + 旧运算名）。
    let legacy = is_java1205_family(form.version);
    let attr = normalize_attribute_id(&form.attribute, !is_modern_nbt_family(form.version));
    let head = format!("attribute {} {attr}", form.target);

    let cmd = match &form.action {
        AttributeAction::Get { scale } => {
            let mut cmd = format!("{head} get");
            if let Some(scale) = scale {
                cmd.push(' ');
                cmd.push_str(&fmt_number_f64(*scale));
            }
            cmd
        }
        AttributeAction::BaseGet { scale } => {
            let mut cmd = format!("{head} base get");
            if let Some(scale) = scale {
                cmd.push(' ');
                cmd.push_str(&fmt_number_f64(*scale));
            }
            cmd
        }
        AttributeAction::BaseSet { value } => format!("{head} base set {}", fmt_number_f64(*value)),
        AttributeAction::ModifierAdd { id, name, value, operation } => {
            if legacy {
                let name = name.clone().unwrap_or_else(|| id.clone());
                format!(
                    "{head} modifier add {id} {name} {} {}",
                    fmt_number_f64(*value),
                    operation_keyword(*operation, true)
                )
            } else {
                format!(
                    "{head} modifier add {} {} {}",
                    namespaced(id),
                    fmt_number_f64(*value),
                    operation_keyword(*operation, false)
                )
            }
        }
        AttributeAction::ModifierRemove { id } => {
            format!("{head} modifier remove {}", if legacy { id.clone() } else { namespaced(id) })
        }
        AttributeAction::ModifierValueGet { id, scale } => {
            let mut cmd =
                format!("{head} modifier value get {}", if legacy { id.clone() } else { namespaced(id) });
            if let Some(scale) = scale {
                cmd.push(' ');
                cmd.push_str(&fmt_number_f64(*scale));
            }
            cmd
        }
    };

    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: GiveVersion = GiveVersion::Java1_21_5;
    const MID: GiveVersion = GiveVersion::Java1_21_4;

    fn form(version: GiveVersion, attribute: &str, action: AttributeAction) -> AttributeForm {
        AttributeForm { version, with_slash: false, target: "@s".to_string(), attribute: attribute.to_string(), action }
    }

    #[test]
    fn base_set_modern_no_generic_prefix() {
        let f = form(MODERN, "max_health", AttributeAction::BaseSet { value: 40.0 });
        assert_eq!(build_attribute_command(&f), "attribute @s minecraft:max_health base set 40");
    }

    #[test]
    fn base_set_mid_with_generic_prefix() {
        let f = form(MID, "max_health", AttributeAction::BaseSet { value: 40.0 });
        assert_eq!(build_attribute_command(&f), "attribute @s minecraft:generic.max_health base set 40");
    }

    #[test]
    fn input_with_prefix_normalized_by_version() {
        let f = form(MODERN, "minecraft:generic.max_health", AttributeAction::BaseSet { value: 40.0 });
        assert_eq!(build_attribute_command(&f), "attribute @s minecraft:max_health base set 40");
    }

    #[test]
    fn modifier_add_121_plus() {
        let f = form(
            MODERN,
            "max_health",
            AttributeAction::ModifierAdd {
                id: "my_buff".to_string(),
                name: None,
                value: 4.0,
                operation: AttributeOperation::Add,
            },
        );
        assert_eq!(
            build_attribute_command(&f),
            "attribute @s minecraft:max_health modifier add minecraft:my_buff 4 add_value"
        );
    }

    #[test]
    fn modifier_add_1205_legacy() {
        let f = form(
            GiveVersion::Java1_20_5,
            "max_health",
            AttributeAction::ModifierAdd {
                id: "uuid-1".to_string(),
                name: Some("buff".to_string()),
                value: 4.0,
                operation: AttributeOperation::MultiplyTotal,
            },
        );
        assert_eq!(
            build_attribute_command(&f),
            "attribute @s minecraft:generic.max_health modifier add uuid-1 buff 4 multiply"
        );
    }

    #[test]
    fn get_with_scale() {
        let f = form(MODERN, "max_health", AttributeAction::Get { scale: Some(0.5) });
        assert_eq!(build_attribute_command(&f), "attribute @s minecraft:max_health get 0.5");
    }
}
