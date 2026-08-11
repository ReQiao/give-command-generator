//! 实体与方块实体 NBT 序列化器（与 /give 的 item 组件格式无关）。
//! 移植自客户端 `src/logic/commands/nbt.ts`。
//!
//! 这是跨指令复用的核心：setblock 的容器物品、summon 的装备/手持物品都走同一套
//! item-in-NBT 结构，只需在此写一次。
//!
//! 实测真值（mc-verifier semantic-probe 1.20.6 / 1.21.5）：
//!   - item-in-NBT：{id:"minecraft:stone", count:5, Slot:0b, components:{...}}
//!     count 小写，id 小写，Slot 大写；Count 大写旧键被静默丢弃
//!   - attributes：1.20.5-1.21.4 用 Attributes[]/Name/Base（generic. 前缀）；1.21.5+ 用 attributes[]/id/base（无前缀）
//!   - active_effects：两版本均用 active_effects[]/id(string)；旧 ActiveEffects/Id(int) 被忽略
//!   - equipment：1.20.5-1.21.4 用 HandItems[]/ArmorItems[]；1.21.5+ 用 equipment{mainhand,...}
//!   - CustomName：SNBT 字符串两版本均有效；裸 JSON compound 仅 1.21.5+ 接受
//!   - 命令方块：Command(大写)/auto(小写)/TrackOutput(大写)

use crate::give::builder::{
    bool_byte, component_id, fmt_number_f64, is_java121_legacy_family, rich_line_to_snbt_string,
    quote, GiveVersion, RichLine,
};
use crate::give::catalog::namespaced;

pub use crate::give::builder::is_modern_nbt_family;

// -----------------------------------------------------------------------
// Item-in-NBT（容器 Items[] / 实体 HandItems[] / 实体 equipment{}）
// -----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NbtEnchantment {
    pub id: String,
    pub level: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NbtItem {
    pub id: String,
    pub count: Option<i64>,
    /// 附魔列表；序列化规则与 /give 完全一致（按版本自动切换 levels 包装），
    /// 调用方（包括 AI 意图）不需要手写 enchantments 组件的原始 SNBT。
    pub enchantments: Option<Vec<NbtEnchantment>>,
    /// 已序列化的 SNBT 值，例如 `'{"text":"x"}'` 或 `1b`。保留插入顺序。
    pub components: Vec<(String, String)>,
}

/// 附魔组件值（不含 key），1.20.5~1.21.1 需要 levels 包装，其余版本直接铺开。
fn serialize_enchantments_value(enchants: &[NbtEnchantment], version: GiveVersion) -> String {
    let entries = enchants
        .iter()
        .filter(|e| !e.id.trim().is_empty())
        .map(|e| format!("{}:{}", component_id(&e.id), e.level))
        .collect::<Vec<_>>()
        .join(",");
    if is_java121_legacy_family(version) {
        format!("{{levels:{{{entries}}}}}")
    } else {
        format!("{{{entries}}}")
    }
}

/// 序列化一个 item-in-NBT（不含 Slot，由调用方决定是否加）。
/// version 缺省时不处理 enchantments（容器物品目前不需要）。
pub fn serialize_item(item: &NbtItem, version: Option<GiveVersion>) -> String {
    let mut parts: Vec<String> =
        vec![format!("id:{}", quote(&namespaced(&item.id))), format!("count:{}", item.count.unwrap_or(1))];

    let mut comps: Vec<(String, String)> = item.components.clone();
    if let (Some(version), Some(enchants)) = (version, item.enchantments.as_ref()) {
        if !enchants.is_empty() {
            comps.push(("enchantments".to_string(), serialize_enchantments_value(enchants, version)));
        }
    }
    if !comps.is_empty() {
        let inner =
            comps.iter().map(|(k, v)| format!("{}:{}", quote(&namespaced(k)), v)).collect::<Vec<_>>().join(",");
        parts.push(format!("components:{{{inner}}}"));
    }
    format!("{{{}}}", parts.join(","))
}

/// 序列化容器 slot（chest / barrel / hopper 等）。Slot 大写 byte，置于首位。
pub fn serialize_container_item(slot: i64, item: &NbtItem, version: Option<GiveVersion>) -> String {
    let serialized = serialize_item(item, version);
    // 去掉开头的 '{'，前面拼上 Slot 字段。
    format!("{{Slot:{slot}b,{}", &serialized[1..])
}

// -----------------------------------------------------------------------
// CustomName（文本组件）
// -----------------------------------------------------------------------

/// 序列化 CustomName 文本。所有版本均用 SNBT 字符串写法（1.21.5+ 也接受裸 JSON，
/// 但统一走 SNBT 以免分叉）。文本内容复用 /give 的富文本序列化管线。
pub fn serialize_custom_name(line: &RichLine, version: GiveVersion) -> String {
    let mut warnings = Vec::new();
    rich_line_to_snbt_string(line, version, &mut warnings)
}

// -----------------------------------------------------------------------
// 属性（Attributes / attributes）
// -----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NbtAttribute {
    /// 属性 id，可带或不带 minecraft: / generic. 前缀，序列化时按版本归一化。
    pub id: String,
    pub base: f64,
}

/// 把各种输入格式归一化为目标格式的属性 id。
/// with_generic=true → "minecraft:generic.max_health"（1.20.5~1.21.4）
/// with_generic=false → "minecraft:max_health"（1.21.5+）
pub fn normalize_attribute_id(raw: &str, with_generic: bool) -> String {
    let mut name = raw.trim();
    if let Some(rest) = name.strip_prefix("minecraft:") {
        name = rest;
    }
    if let Some(rest) = name.strip_prefix("generic.") {
        name = rest;
    }
    if with_generic { format!("minecraft:generic.{name}") } else { format!("minecraft:{name}") }
}

/// 序列化属性列表（modern 决定新旧键名，见文件头真值表）。
pub fn serialize_attributes(attrs: &[NbtAttribute], modern: bool) -> String {
    if attrs.is_empty() {
        return String::new();
    }
    if modern {
        let entries = attrs
            .iter()
            .map(|a| format!("{{id:{},base:{}d}}", quote(&normalize_attribute_id(&a.id, false)), fmt_number_f64(a.base)))
            .collect::<Vec<_>>()
            .join(",");
        format!("attributes:[{entries}]")
    } else {
        let entries = attrs
            .iter()
            .map(|a| {
                format!("{{Name:{},Base:{}d}}", quote(&normalize_attribute_id(&a.id, true)), fmt_number_f64(a.base))
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("Attributes:[{entries}]")
    }
}

// -----------------------------------------------------------------------
// 状态效果（active_effects）
// -----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NbtEffect {
    pub id: String,
    pub duration: Option<i64>,
    pub amplifier: Option<i64>,
    pub show_particles: Option<bool>,
}

/// 序列化状态效果列表。两版本均用 active_effects(小写)/id(string)/amplifier(byte)。
pub fn serialize_effects(effects: &[NbtEffect]) -> String {
    if effects.is_empty() {
        return String::new();
    }
    let entries = effects
        .iter()
        .map(|e| {
            let duration = e.duration.unwrap_or(200);
            let amplifier = e.amplifier.unwrap_or(0);
            let show_particles = e.show_particles.unwrap_or(true);
            format!(
                "{{id:{},duration:{},amplifier:{}b,show_particles:{}}}",
                quote(&namespaced(&e.id)),
                duration,
                amplifier,
                bool_byte(show_particles)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("active_effects:[{entries}]")
}

// -----------------------------------------------------------------------
// 实体装备槽（HandItems / ArmorItems / equipment）
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct NbtEquipment {
    pub mainhand: Option<NbtItem>,
    pub offhand: Option<NbtItem>,
    pub head: Option<NbtItem>,
    pub chest: Option<NbtItem>,
    pub legs: Option<NbtItem>,
    pub feet: Option<NbtItem>,
}

/// 序列化实体装备，返回 0~2 个 NBT 片段。
///   1.20.5~1.21.4：HandItems:[mainhand,offhand] + ArmorItems:[feet,legs,chest,head]
///   1.21.5+：      equipment:{mainhand:{...},...}
/// version 传入时装备物品上的 enchantments 会按版本正确序列化（见 serialize_item）。
pub fn serialize_equipment(eq: &NbtEquipment, modern: bool, version: Option<GiveVersion>) -> Vec<String> {
    if modern {
        let slots: Vec<(&str, &Option<NbtItem>)> = vec![
            ("mainhand", &eq.mainhand),
            ("offhand", &eq.offhand),
            ("head", &eq.head),
            ("chest", &eq.chest),
            ("legs", &eq.legs),
            ("feet", &eq.feet),
        ];
        let filled: Vec<(&str, &NbtItem)> =
            slots.into_iter().filter_map(|(name, item)| item.as_ref().map(|i| (name, i))).collect();
        if filled.is_empty() {
            return Vec::new();
        }
        let inner = filled
            .iter()
            .map(|(name, item)| format!("{name}:{}", serialize_item(item, version)))
            .collect::<Vec<_>>()
            .join(",");
        return vec![format!("equipment:{{{inner}}}")];
    }

    // 旧格式是定长数组，空槽位必须占位 {}；整组都空时干脆不输出该键。
    let slot = |item: &Option<NbtItem>| match item {
        Some(i) => serialize_item(i, version),
        None => "{}".to_string(),
    };
    let mut parts = Vec::new();
    if eq.mainhand.is_some() || eq.offhand.is_some() {
        parts.push(format!("HandItems:[{},{}]", slot(&eq.mainhand), slot(&eq.offhand)));
    }
    if eq.feet.is_some() || eq.legs.is_some() || eq.chest.is_some() || eq.head.is_some() {
        parts.push(format!(
            "ArmorItems:[{},{},{},{}]",
            slot(&eq.feet),
            slot(&eq.legs),
            slot(&eq.chest),
            slot(&eq.head)
        ));
    }
    parts
}
