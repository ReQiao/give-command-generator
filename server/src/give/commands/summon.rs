//! `/summon` 指令构建器。移植自客户端 `src/logic/commands/summon.ts`。
//!
//! 语法（1.20.5+）：
//!   summon <entity_type> [<x> <y> <z>] [{nbt_compound}]
//!
//! 版本敏感点全部下沉到 `commands::nbt`（属性 / 装备 / CustomName），
//! 这里只负责组装。真值表见 nbt.rs 文件头。

use crate::give::builder::{bool_byte, fmt_number_f64, is_modern_nbt_family, GiveVersion, RichLine};
use crate::give::catalog::namespaced;
use crate::give::commands::nbt::{
    serialize_attributes, serialize_custom_name, serialize_effects, serialize_equipment, NbtAttribute,
    NbtEffect, NbtEquipment,
};

#[derive(Debug, Clone, Default)]
pub struct SummonPassenger {
    pub entity_type: String,
    pub no_ai: bool,
    pub silent: bool,
    pub custom_name: Option<RichLine>,
    /// 已序列化的附加 SNBT 片段（逗号分隔，不含外层花括号）。
    pub extra_nbt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SummonForm {
    pub version: GiveVersion,
    pub with_slash: bool,
    pub entity_type: String,
    /// 坐标，支持 "0" / "~" / "~1" / "^1"；三者需同时提供，否则视为不指定。
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
    pub custom_name: Option<RichLine>,
    pub no_ai: bool,
    pub silent: bool,
    pub persistence_required: bool,
    pub invulnerable: bool,
    pub no_gravity: bool,
    pub glowing: bool,
    /// 出生朝向 [yaw, pitch]（度）。只影响朝向，不影响移动方向。
    pub rotation: Option<(f64, f64)>,
    /// 当前生命值。改 max_health 属性只会改「上限」，不改「当前值」——
    /// 不配这个字段，生物仍按旧上限（通常 20）的血量生成，看起来像没生效。
    /// 想要生物一出生就满新血量，必须把这个设成和 max_health 属性相同的值。
    pub health: Option<f64>,
    pub tags: Vec<String>,
    pub attributes: Vec<NbtAttribute>,
    pub effects: Vec<NbtEffect>,
    pub equipment: Option<NbtEquipment>,
    pub passengers: Vec<SummonPassenger>,
    /// 已序列化的附加 SNBT 片段（逗号分隔，不含外层花括号）。
    pub extra_nbt: Option<String>,
}

pub fn build_summon_command(form: &SummonForm) -> String {
    let modern = is_modern_nbt_family(form.version);
    let nbt_parts = build_nbt_parts(form, modern);
    let has_nbt = !nbt_parts.is_empty();
    let has_pos = form.x.is_some() && form.y.is_some() && form.z.is_some();

    let mut cmd = format!("summon {}", namespaced(&form.entity_type));
    // NBT 是第 5 个位置参数：要带 NBT 就必须先补出坐标。
    if has_pos || has_nbt {
        cmd.push(' ');
        cmd.push_str(form.x.as_deref().unwrap_or("~"));
        cmd.push(' ');
        cmd.push_str(form.y.as_deref().unwrap_or("~"));
        cmd.push(' ');
        cmd.push_str(form.z.as_deref().unwrap_or("~"));
    }
    if has_nbt {
        cmd.push_str(" {");
        cmd.push_str(&nbt_parts.join(","));
        cmd.push('}');
    }
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

fn build_nbt_parts(form: &SummonForm, modern: bool) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(custom_name) = &form.custom_name {
        if !custom_name.is_empty() {
            parts.push(format!("CustomName:{}", serialize_custom_name(custom_name, form.version)));
        }
    }
    if form.no_ai {
        parts.push(format!("NoAI:{}", bool_byte(true)));
    }
    if form.silent {
        parts.push(format!("Silent:{}", bool_byte(true)));
    }
    if form.persistence_required {
        parts.push(format!("PersistenceRequired:{}", bool_byte(true)));
    }
    if form.invulnerable {
        parts.push(format!("Invulnerable:{}", bool_byte(true)));
    }
    if form.no_gravity {
        parts.push(format!("NoGravity:{}", bool_byte(true)));
    }
    if form.glowing {
        parts.push(format!("Glowing:{}", bool_byte(true)));
    }
    if let Some((yaw, pitch)) = form.rotation {
        parts.push(format!("Rotation:[{}f,{}f]", fmt_number_f64(yaw), fmt_number_f64(pitch)));
    }
    if let Some(health) = form.health {
        parts.push(format!("Health:{}f", fmt_number_f64(health)));
    }

    if !form.tags.is_empty() {
        let tags = form.tags.iter().map(|t| crate::give::builder::quote(t)).collect::<Vec<_>>().join(",");
        parts.push(format!("Tags:[{tags}]"));
    }
    if !form.attributes.is_empty() {
        parts.push(serialize_attributes(&form.attributes, modern));
    }
    if !form.effects.is_empty() {
        parts.push(serialize_effects(&form.effects));
    }
    if let Some(equipment) = &form.equipment {
        parts.extend(serialize_equipment(equipment, modern, Some(form.version)));
    }
    if !form.passengers.is_empty() {
        let passengers = form
            .passengers
            .iter()
            .map(|p| build_passenger_nbt(p, form.version))
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("Passengers:[{passengers}]"));
    }
    if let Some(extra) = &form.extra_nbt {
        let trimmed = extra.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }

    parts
}

fn build_passenger_nbt(p: &SummonPassenger, version: GiveVersion) -> String {
    let mut inner: Vec<String> = vec![format!("id:{}", crate::give::builder::quote(&namespaced(&p.entity_type)))];
    if p.no_ai {
        inner.push(format!("NoAI:{}", bool_byte(true)));
    }
    if p.silent {
        inner.push(format!("Silent:{}", bool_byte(true)));
    }
    if let Some(custom_name) = &p.custom_name {
        if !custom_name.is_empty() {
            inner.push(format!("CustomName:{}", serialize_custom_name(custom_name, version)));
        }
    }
    if let Some(extra) = &p.extra_nbt {
        let trimmed = extra.trim();
        if !trimmed.is_empty() {
            inner.push(trimmed.to_string());
        }
    }
    format!("{{{}}}", inner.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::give::commands::nbt::{NbtEnchantment, NbtItem};
    use serde_json::json;

    const MODERN: GiveVersion = GiveVersion::Java1_21_5;
    const MID: GiveVersion = GiveVersion::Java1_21_4;

    fn base(version: GiveVersion, entity_type: &str) -> SummonForm {
        SummonForm {
            version,
            with_slash: false,
            entity_type: entity_type.to_string(),
            x: None,
            y: None,
            z: None,
            custom_name: None,
            no_ai: false,
            silent: false,
            persistence_required: false,
            invulnerable: false,
            no_gravity: false,
            glowing: false,
            rotation: None,
            health: None,
            tags: Vec::new(),
            attributes: Vec::new(),
            effects: Vec::new(),
            equipment: None,
            passengers: Vec::new(),
            extra_nbt: None,
        }
    }

    #[test]
    fn basic_no_coords_no_nbt() {
        let f = base(MODERN, "pig");
        assert_eq!(build_summon_command(&f), "summon minecraft:pig");
    }

    #[test]
    fn specific_coords() {
        let mut f = base(MODERN, "pig");
        f.x = Some("0".to_string());
        f.y = Some("64".to_string());
        f.z = Some("0".to_string());
        assert_eq!(build_summon_command(&f), "summon minecraft:pig 0 64 0");
    }

    #[test]
    fn nbt_present_auto_fills_coords() {
        let mut f = base(MODERN, "zombie");
        f.no_ai = true;
        f.silent = true;
        assert_eq!(build_summon_command(&f), "summon minecraft:zombie ~ ~ ~ {NoAI:1b,Silent:1b}");
    }

    #[test]
    fn custom_name_snbt_string() {
        let mut f = base(MODERN, "zombie");
        f.custom_name = Some(vec![json!({"text": "Boss", "color": "red"})]);
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {CustomName:'[{"text":"Boss","color":"red"}]'}"#
        );
    }

    #[test]
    fn tags() {
        let mut f = base(MODERN, "pig");
        f.tags = vec!["a".to_string(), "b".to_string()];
        assert_eq!(build_summon_command(&f), r#"summon minecraft:pig ~ ~ ~ {Tags:["a","b"]}"#);
    }

    #[test]
    fn attributes_121_5_plus() {
        let mut f = base(MODERN, "zombie");
        f.attributes = vec![NbtAttribute { id: "max_health".to_string(), base: 40.0 }];
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {attributes:[{id:"minecraft:max_health",base:40d}]}"#
        );
    }

    #[test]
    fn attributes_1214() {
        let mut f = base(MID, "zombie");
        f.attributes = vec![NbtAttribute { id: "max_health".to_string(), base: 40.0 }];
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {Attributes:[{Name:"minecraft:generic.max_health",Base:40d}]}"#
        );
    }

    #[test]
    fn effects_same_format_both_versions() {
        let mut f = base(MID, "zombie");
        f.effects = vec![NbtEffect { id: "speed".to_string(), duration: Some(200), amplifier: Some(1), show_particles: None }];
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {active_effects:[{id:"minecraft:speed",duration:200,amplifier:1b,show_particles:1b}]}"#
        );
    }

    #[test]
    fn equipment_121_5_plus_compound() {
        let mut f = base(MODERN, "zombie");
        f.equipment = Some(NbtEquipment {
            mainhand: Some(NbtItem { id: "diamond_sword".to_string(), ..Default::default() }),
            ..Default::default()
        });
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {equipment:{mainhand:{id:"minecraft:diamond_sword",count:1}}}"#
        );
    }

    #[test]
    fn equipment_1214_handitems_no_armoritems() {
        let mut f = base(MID, "zombie");
        f.equipment = Some(NbtEquipment {
            mainhand: Some(NbtItem { id: "diamond_sword".to_string(), ..Default::default() }),
            ..Default::default()
        });
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {HandItems:[{id:"minecraft:diamond_sword",count:1},{}]}"#
        );
    }

    #[test]
    fn equipment_1214_armor_fixed_order() {
        let mut f = base(MID, "zombie");
        f.equipment = Some(NbtEquipment {
            head: Some(NbtItem { id: "diamond_helmet".to_string(), ..Default::default() }),
            ..Default::default()
        });
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {ArmorItems:[{},{},{},{id:"minecraft:diamond_helmet",count:1}]}"#
        );
    }

    #[test]
    fn passengers() {
        let mut f = base(MODERN, "pig");
        f.passengers = vec![SummonPassenger { entity_type: "chicken".to_string(), ..Default::default() }];
        assert_eq!(build_summon_command(&f), r#"summon minecraft:pig ~ ~ ~ {Passengers:[{id:"minecraft:chicken"}]}"#);
    }

    #[test]
    fn health_alone() {
        let mut f = base(MODERN, "zombie");
        f.health = Some(40.0);
        assert_eq!(build_summon_command(&f), "summon minecraft:zombie ~ ~ ~ {Health:40f}");
    }

    #[test]
    fn health_and_max_health_attribute_combo() {
        let mut f = base(MODERN, "zombie");
        f.attributes = vec![NbtAttribute { id: "max_health".to_string(), base: 40.0 }];
        f.health = Some(40.0);
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {Health:40f,attributes:[{id:"minecraft:max_health",base:40d}]}"#
        );
    }

    #[test]
    fn rotation_yaw_pitch() {
        let mut f = base(MODERN, "zombie");
        f.rotation = Some((90.0, 0.0));
        assert_eq!(build_summon_command(&f), "summon minecraft:zombie ~ ~ ~ {Rotation:[90f,0f]}");
    }

    #[test]
    fn equipment_enchantments_121_5_plus_no_levels_wrap() {
        let mut f = base(MODERN, "zombie");
        f.equipment = Some(NbtEquipment {
            mainhand: Some(NbtItem {
                id: "diamond_sword".to_string(),
                enchantments: Some(vec![NbtEnchantment { id: "sharpness".to_string(), level: 5 }]),
                ..Default::default()
            }),
            ..Default::default()
        });
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {equipment:{mainhand:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{sharpness:5}}}}}"#
        );
    }

    #[test]
    fn equipment_enchantments_java_1_21_levels_wrap() {
        let mut f = base(GiveVersion::Java1_21, "zombie");
        f.equipment = Some(NbtEquipment {
            mainhand: Some(NbtItem {
                id: "diamond_sword".to_string(),
                enchantments: Some(vec![NbtEnchantment { id: "sharpness".to_string(), level: 5 }]),
                ..Default::default()
            }),
            ..Default::default()
        });
        assert_eq!(
            build_summon_command(&f),
            r#"summon minecraft:zombie ~ ~ ~ {HandItems:[{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{levels:{sharpness:5}}}},{}]}"#
        );
    }
}
