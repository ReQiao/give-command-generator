//! give 指令核心构建器。移植自客户端 `src/logic/builder.ts`。
//!
//! 本文件包含：`GiveVersion` 类型骨架（早前阶段已落地）+ 本阶段移植的
//! `GiveForm`/`createDefaultForm`/`normalizeForm`/`buildGiveCommand` 全套核心逻辑、
//! 富文本组件序列化（`resolveTextProfile`/`richLineToSnbtString` 等）、颜色渐变工具、
//! 版本判断族函数。`mapCatalog`/`namespaced` 复用 `catalog.rs`，不在这里重复实现。
//!
//! 尚未移植：13 个 `commands/*` 构建器、dispatch 分派、`parseAiContent`——这些是
//! AI 意图 -> `GiveForm` 的转换层，不属于本阶段任务范围。

use serde_json::Value;

use crate::give::catalog::{
    map_catalog, namespaced, ATTRIBUTES, BEDROCK_BLOCKS, BEDROCK_ITEMS, BLOCKS, ENCHANTS, ITEMS,
};

/// 对应客户端 `src/logic/builder.ts::GiveVersion`——13 个变体必须逐一对应，
/// 顺序/拼写任何差异都会导致服务器和客户端对"这是哪个版本"的理解对不上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiveVersion {
    #[serde(rename = "java_1_20_5")]
    Java1_20_5,
    #[serde(rename = "java_1_21")]
    Java1_21,
    #[serde(rename = "java_1_21_1")]
    Java1_21_1,
    #[serde(rename = "java_1_21_2")]
    Java1_21_2,
    #[serde(rename = "java_1_21_3")]
    Java1_21_3,
    #[serde(rename = "java_1_21_4")]
    Java1_21_4,
    #[serde(rename = "java_1_21_5")]
    Java1_21_5,
    #[serde(rename = "java_1_21_6")]
    Java1_21_6,
    #[serde(rename = "java_1_21_9")]
    Java1_21_9,
    #[serde(rename = "java_1_21_11_plus")]
    Java1_21_11Plus,
    #[serde(rename = "java_26_1")]
    Java26_1,
    #[serde(rename = "java_26_2_plus")]
    Java26_2Plus,
    #[serde(rename = "bedrock")]
    Bedrock,
}

// =====================================================================================
// 富文本组件模型
// =====================================================================================
//
// 与客户端不同，这里不用强类型 enum 表达 RichComponent——TS 原版本来就没有运行时校验
// （normalizeForm 只检查外层数组是不是数组，内部组件形状完全照抄客户端/AI 传来的数据），
// 用 serde_json::Value 原样保留同样的"宽松"特性，也避免了为 13 种组件变体各写一套
// 序列化/反序列化代码。componentToJson 的等价物（component_to_json）直接在 Value 上
// 按字段名读取，和 TS 里 `anyRun.xxx` 的写法一一对应。

/// 一行富文本 = 一串组件（每个组件是一个宽松的 JSON 对象）。
pub type RichLine = Vec<Value>;

#[derive(Debug, Clone)]
pub struct EnchantRow {
    pub id: String,
    pub level: Value,
}

#[derive(Debug, Clone)]
pub struct AttributeRow {
    pub r#type: String,
    pub amount: Value,
    pub slot: String,
    pub operation: String,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct BlockLimitRow {
    pub block: String,
    pub r#type: String,
}

#[derive(Debug, Clone)]
pub enum EffectEntry {
    Id(String),
    Full {
        id: String,
        duration: Value,
        amplifier: Value,
        show_particles: Option<bool>,
        show_icon: Option<bool>,
    },
}

#[derive(Debug, Clone)]
pub struct EffectGroup {
    pub r#type: String,
    pub probability_percent: Value,
    pub diameter: Value,
    pub effects: Vec<EffectEntry>,
}

#[derive(Debug, Clone)]
pub struct ToolRuleRow {
    /// `string[] | string`（原样保留，build 时再 splitCsv/取数组）。
    pub blocks: Value,
    pub speed: Value,
    pub correct_for_drops: String,
}

/// 对应客户端 `src/logic/builder.ts::GiveForm`。字段名从 camelCase 改成 Rust 惯用的
/// snake_case，含义和默认值逐一对应，注释里标出原 TS 字段名以便对照。
#[derive(Debug, Clone)]
pub struct GiveForm {
    pub version: GiveVersion,
    pub target: String,
    pub item: String,
    pub count: i64,
    /// TS: withSlash
    pub with_slash: bool,
    /// TS: templateName
    pub template_name: String,
    /// TS: bedrockDataValue
    pub bedrock_data_value: i64,
    /// TS: bedrockItemLock
    pub bedrock_item_lock: String,
    /// TS: bedrockKeepOnDeath
    pub bedrock_keep_on_death: bool,
    /// TS: displayName
    pub display_name: Vec<RichLine>,
    /// TS: itemName
    pub item_name: Vec<RichLine>,
    pub lore: Vec<RichLine>,
    pub rarity: String,
    pub glint: String,
    pub enchantments: Vec<EnchantRow>,
    pub attributes: Vec<AttributeRow>,
    /// TS: blockLimits
    pub block_limits: Vec<BlockLimitRow>,
    pub unbreakable: bool,
    pub glider: bool,
    /// TS: deathProtection
    pub death_protection: bool,
    /// TS: deathEffects
    pub death_effects: Vec<EffectGroup>,
    /// TS: damageEnabled
    pub damage_enabled: bool,
    pub damage: i64,
    /// TS: maxDamageEnabled
    pub max_damage_enabled: bool,
    /// TS: maxDamage
    pub max_damage: i64,
    /// TS: stackEnabled
    pub stack_enabled: bool,
    /// TS: maxStackSize
    pub max_stack_size: i64,
    /// TS: repairEnabled
    pub repair_enabled: bool,
    /// TS: repairCost
    pub repair_cost: i64,
    /// TS: hiddenComponents
    pub hidden_components: String,
    /// TS: foodEnabled
    pub food_enabled: bool,
    pub nutrition: i64,
    pub saturation: f64,
    /// TS: alwaysEat
    pub always_eat: String,
    /// TS: consumableEnabled
    pub consumable_enabled: bool,
    /// TS: consumeSeconds
    pub consume_seconds: f64,
    /// TS: consumeSound
    pub consume_sound: String,
    /// TS: consumeParticles
    pub consume_particles: String,
    /// TS: consumeEffects
    pub consume_effects: Vec<EffectGroup>,
    /// TS: toolEnabled
    pub tool_enabled: bool,
    /// TS: defaultMiningSpeed
    pub default_mining_speed: f64,
    /// TS: damagePerBlock
    pub damage_per_block: i64,
    /// TS: toolRules
    pub tool_rules: Vec<ToolRuleRow>,
    /// TS: customData —— custom_data 组件的原始 SNBT 复合内容（含外层花括号）。
    /// 主要给 AI 模式用，见 TS 侧同名字段的详细注释。
    pub custom_data: String,
}

pub fn create_default_form() -> GiveForm {
    GiveForm {
        version: GiveVersion::Java1_21_11Plus,
        target: "@a".to_string(),
        item: "石头".to_string(),
        count: 1,
        with_slash: false,
        template_name: "未命名模板".to_string(),
        bedrock_data_value: 0,
        bedrock_item_lock: "不设置".to_string(),
        bedrock_keep_on_death: false,
        display_name: Vec::new(),
        item_name: Vec::new(),
        lore: Vec::new(),
        rarity: "不设置".to_string(),
        glint: "默认".to_string(),
        enchantments: Vec::new(),
        attributes: Vec::new(),
        block_limits: Vec::new(),
        unbreakable: false,
        glider: false,
        death_protection: false,
        death_effects: Vec::new(),
        damage_enabled: false,
        damage: 0,
        max_damage_enabled: false,
        max_damage: 1,
        stack_enabled: false,
        max_stack_size: 1,
        repair_enabled: false,
        repair_cost: 0,
        hidden_components: String::new(),
        food_enabled: false,
        nutrition: 0,
        saturation: 0.0,
        always_eat: "默认".to_string(),
        consumable_enabled: false,
        consume_seconds: 0.0,
        consume_sound: String::new(),
        consume_particles: "默认".to_string(),
        consume_effects: Vec::new(),
        tool_enabled: false,
        default_mining_speed: 1.0,
        damage_per_block: 0,
        tool_rules: Vec::new(),
        custom_data: String::new(),
    }
}

// ---------------- normalizeForm 及其辅助解析函数 ----------------
// AI 产出的意图数据经常缺字段/字段类型不对，这里的策略和 TS 原版一致：
// 每个字段独立兜底，缺失就用 createDefaultForm() 的默认值，绝不因为一个字段
// 有问题就让整个表单解析失败。

fn parse_enchant_row(v: &Value) -> EnchantRow {
    EnchantRow {
        id: v.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
        level: v.get("level").cloned().unwrap_or(Value::Null),
    }
}

fn parse_attribute_row(v: &Value) -> AttributeRow {
    AttributeRow {
        r#type: v.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
        amount: v.get("amount").cloned().unwrap_or(Value::Null),
        slot: v.get("slot").and_then(Value::as_str).unwrap_or("").to_string(),
        operation: v.get("operation").and_then(Value::as_str).unwrap_or("").to_string(),
        id: v.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
    }
}

fn parse_block_limit_row(v: &Value) -> BlockLimitRow {
    BlockLimitRow {
        block: v.get("block").and_then(Value::as_str).unwrap_or("").to_string(),
        r#type: v.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
    }
}

fn parse_effect_entry(v: &Value) -> Option<EffectEntry> {
    match v {
        Value::String(s) => Some(EffectEntry::Id(s.clone())),
        Value::Object(_) => Some(EffectEntry::Full {
            id: v.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            duration: v.get("duration").cloned().unwrap_or(Value::Null),
            amplifier: v.get("amplifier").cloned().unwrap_or(Value::Null),
            show_particles: v.get("show_particles").and_then(Value::as_bool),
            show_icon: v.get("show_icon").and_then(Value::as_bool),
        }),
        _ => None,
    }
}

fn parse_effect_group(v: &Value) -> EffectGroup {
    EffectGroup {
        r#type: v.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
        probability_percent: v.get("probability_percent").cloned().unwrap_or(Value::Null),
        diameter: v.get("diameter").cloned().unwrap_or(Value::Null),
        effects: v
            .get("effects")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(parse_effect_entry).collect())
            .unwrap_or_default(),
    }
}

fn parse_tool_rule_row(v: &Value) -> ToolRuleRow {
    ToolRuleRow {
        blocks: v.get("blocks").cloned().unwrap_or(Value::Null),
        speed: v.get("speed").cloned().unwrap_or(Value::Null),
        correct_for_drops: v.get("correct_for_drops").and_then(Value::as_str).unwrap_or("").to_string(),
    }
}

fn parse_rich_line(v: &Value) -> RichLine {
    v.as_array().cloned().unwrap_or_default()
}

/// 对应客户端 `normalizeForm(value: unknown): GiveForm`。
pub fn normalize_form(value: &Value) -> GiveForm {
    let fallback = create_default_form();
    let data = match value.as_object() {
        Some(o) => o,
        None => return fallback,
    };
    let get = |k: &str| data.get(k);
    let str_or = |k: &str, fb: &String| get(k).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| fb.clone());
    let bool_or = |k: &str, fb: bool| get(k).map(truthy).unwrap_or(fb);
    let arr_or = |k: &str| get(k).and_then(Value::as_array).cloned().unwrap_or_default();

    GiveForm {
        version: get("version").and_then(Value::as_str).map(normalize_version).unwrap_or(fallback.version),
        target: str_or("target", &fallback.target),
        item: str_or("item", &fallback.item),
        count: get("count").map(|v| normalize_int(v, fallback.count, 1)).unwrap_or(fallback.count),
        with_slash: bool_or("withSlash", fallback.with_slash),
        template_name: str_or("templateName", &fallback.template_name),
        bedrock_data_value: get("bedrockDataValue").map(|v| normalize_int(v, fallback.bedrock_data_value, 0)).unwrap_or(fallback.bedrock_data_value),
        bedrock_item_lock: str_or("bedrockItemLock", &fallback.bedrock_item_lock),
        bedrock_keep_on_death: bool_or("bedrockKeepOnDeath", fallback.bedrock_keep_on_death),
        display_name: arr_or("displayName").iter().map(parse_rich_line).collect(),
        item_name: arr_or("itemName").iter().map(parse_rich_line).collect(),
        lore: arr_or("lore").iter().map(parse_rich_line).collect(),
        rarity: str_or("rarity", &fallback.rarity),
        glint: str_or("glint", &fallback.glint),
        enchantments: arr_or("enchantments").iter().map(parse_enchant_row).collect(),
        attributes: arr_or("attributes").iter().map(parse_attribute_row).collect(),
        block_limits: arr_or("blockLimits").iter().map(parse_block_limit_row).collect(),
        unbreakable: bool_or("unbreakable", fallback.unbreakable),
        glider: bool_or("glider", fallback.glider),
        death_protection: bool_or("deathProtection", fallback.death_protection),
        death_effects: arr_or("deathEffects").iter().map(parse_effect_group).collect(),
        damage_enabled: bool_or("damageEnabled", fallback.damage_enabled),
        damage: get("damage").map(|v| normalize_int(v, fallback.damage, 0)).unwrap_or(fallback.damage),
        max_damage_enabled: bool_or("maxDamageEnabled", fallback.max_damage_enabled),
        max_damage: get("maxDamage").map(|v| normalize_int(v, fallback.max_damage, 1)).unwrap_or(fallback.max_damage),
        stack_enabled: bool_or("stackEnabled", fallback.stack_enabled),
        max_stack_size: get("maxStackSize").map(|v| normalize_int(v, fallback.max_stack_size, 1)).unwrap_or(fallback.max_stack_size),
        repair_enabled: bool_or("repairEnabled", fallback.repair_enabled),
        repair_cost: get("repairCost").map(|v| normalize_int(v, fallback.repair_cost, 0)).unwrap_or(fallback.repair_cost),
        hidden_components: str_or("hiddenComponents", &fallback.hidden_components),
        food_enabled: bool_or("foodEnabled", fallback.food_enabled),
        nutrition: get("nutrition").map(|v| normalize_int(v, fallback.nutrition, 0)).unwrap_or(fallback.nutrition),
        saturation: get("saturation").map(|v| normalize_number(v, fallback.saturation, 0.0)).unwrap_or(fallback.saturation),
        always_eat: str_or("alwaysEat", &fallback.always_eat),
        consumable_enabled: bool_or("consumableEnabled", fallback.consumable_enabled),
        consume_seconds: get("consumeSeconds").map(|v| normalize_number(v, fallback.consume_seconds, 0.0)).unwrap_or(fallback.consume_seconds),
        consume_sound: str_or("consumeSound", &fallback.consume_sound),
        consume_particles: str_or("consumeParticles", &fallback.consume_particles),
        consume_effects: arr_or("consumeEffects").iter().map(parse_effect_group).collect(),
        tool_enabled: bool_or("toolEnabled", fallback.tool_enabled),
        default_mining_speed: get("defaultMiningSpeed").map(|v| normalize_number(v, fallback.default_mining_speed, 0.0)).unwrap_or(fallback.default_mining_speed),
        damage_per_block: get("damagePerBlock").map(|v| normalize_int(v, fallback.damage_per_block, 0)).unwrap_or(fallback.damage_per_block),
        tool_rules: arr_or("toolRules").iter().map(parse_tool_rule_row).collect(),
        custom_data: get("customData").and_then(Value::as_str).map(str::to_string).unwrap_or(fallback.custom_data),
    }
}

// =====================================================================================
// 版本档案 & buildGiveCommand
// =====================================================================================

#[derive(Debug, Clone, Copy)]
struct ModernProfile {
    text_as_snbt_string: bool,
    adventure_predicate_wrapper: bool,
    supports_tooltip_display: bool,
    supports_consumable: bool,
    supports_glider: bool,
    supports_death_protection: bool,
    supports_attribute_modifiers: bool,
}

const MODERN_PROFILE: ModernProfile = ModernProfile {
    text_as_snbt_string: false,
    adventure_predicate_wrapper: false,
    supports_tooltip_display: true,
    supports_consumable: true,
    supports_glider: true,
    supports_death_protection: true,
    supports_attribute_modifiers: true,
};

const JAVA_1_21_2_PROFILE: ModernProfile = ModernProfile {
    text_as_snbt_string: true,
    adventure_predicate_wrapper: true,
    supports_tooltip_display: false,
    supports_consumable: true,
    supports_glider: true,
    supports_death_protection: true,
    supports_attribute_modifiers: true,
};

const JAVA_1_20_5_PROFILE: ModernProfile = ModernProfile {
    text_as_snbt_string: true,
    adventure_predicate_wrapper: true,
    supports_tooltip_display: false,
    supports_consumable: false,
    supports_glider: false,
    supports_death_protection: false,
    supports_attribute_modifiers: false,
};

/// 对应客户端 `buildGiveCommand(form, warnings=[])`。
pub fn build_give_command(form: &GiveForm, warnings: &mut Vec<String>) -> String {
    if form.version == GiveVersion::Bedrock {
        return build_bedrock(form);
    }
    if is_java121_legacy_family(form.version) {
        return build_java121_legacy(form, warnings);
    }
    if is_java1205_family(form.version) {
        return build_modern_family(form, &JAVA_1_20_5_PROFILE, warnings);
    }
    if is_java1212_family(form.version) {
        return build_modern_family(form, &JAVA_1_21_2_PROFILE, warnings);
    }
    build_modern_family(form, &MODERN_PROFILE, warnings)
}

fn build_modern_family(form: &GiveForm, profile: &ModernProfile, warnings: &mut Vec<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let tp = resolve_text_profile(form.version);

    if let Some(line) = form.display_name.first() {
        parts.push(format!("custom_name={}", serialize_text(line, profile.text_as_snbt_string, &tp, warnings)));
    }
    if let Some(line) = form.item_name.first() {
        parts.push(format!("item_name={}", serialize_text(line, profile.text_as_snbt_string, &tp, warnings)));
    }
    if !form.lore.is_empty() {
        let items: Vec<String> = if profile.text_as_snbt_string {
            form.lore.iter().map(|line| serialize_text(line, true, &tp, warnings)).collect()
        } else {
            form.lore.iter().map(|line| json_rich_line_slice(line, &tp, warnings).stringify()).collect()
        };
        parts.push(format!("lore=[{}]", items.join(",")));
    }

    let rarity = pair_value(RARITIES, &form.rarity);
    if rarity != "none" {
        parts.push(format!("rarity={rarity}"));
    }

    if form.glint != "默认" {
        parts.push(format!("enchantment_glint_override={}", if form.glint == "开启" { "true" } else { "false" }));
    }

    let enchants: Vec<String> = form
        .enchantments
        .iter()
        .filter(|r| !r.id.trim().is_empty())
        .map(|r| format!("{}:{}", component_id(&map_catalog(ENCHANTS, &r.id)), normalize_int(&r.level, 1, 1)))
        .collect();
    if !enchants.is_empty() {
        parts.push(format!("enchantments={{{}}}", enchants.join(",")));
    }

    if profile.supports_attribute_modifiers {
        let attrs: Vec<String> = form
            .attributes
            .iter()
            .filter(|r| !r.r#type.trim().is_empty())
            .map(|r| {
                let mut fields = vec![
                    format!("type:{}", component_id(&map_catalog(ATTRIBUTES, &r.r#type))),
                    format!("amount:{}", fmt_number(&r.amount)),
                ];
                let slot_input = if r.slot.trim().is_empty() { "任意" } else { r.slot.as_str() };
                let slot = pair_value(SLOTS, slot_input);
                if !slot.is_empty() && slot != "any" {
                    fields.push(format!("slot:{slot}"));
                }
                let id_val = if r.id.trim().is_empty() { crypto_id() } else { r.id.clone() };
                fields.push(format!("id:{}", quote(&id_val)));
                let op_input = if r.operation.trim().is_empty() { "加算" } else { r.operation.as_str() };
                fields.push(format!("operation:{}", pair_value(OPERATIONS, op_input)));
                format!("{{{}}}", fields.join(","))
            })
            .collect();
        if !attrs.is_empty() {
            parts.push(format!("attribute_modifiers=[{}]", attrs.join(",")));
        }
    }

    let place: Vec<&BlockLimitRow> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "place" | "both"))
        .collect();
    let brk: Vec<&BlockLimitRow> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "break" | "both"))
        .collect();
    let place_predicates = block_predicate_list(&place);
    let break_predicates = block_predicate_list(&brk);
    let wrap = |preds: &[String]| -> String {
        if profile.adventure_predicate_wrapper {
            format!("{{predicates:[{}]}}", preds.join(","))
        } else {
            format!("[{}]", preds.join(","))
        }
    };
    if !place_predicates.is_empty() {
        parts.push(format!("can_place_on={}", wrap(&place_predicates)));
    }
    if !break_predicates.is_empty() {
        parts.push(format!("can_break={}", wrap(&break_predicates)));
    }

    if form.unbreakable {
        parts.push("unbreakable={}".to_string());
    }
    if !form.custom_data.trim().is_empty() {
        parts.push(format!("custom_data={}", form.custom_data.trim()));
    }
    if profile.supports_glider && form.glider {
        parts.push("glider={}".to_string());
    }

    if profile.supports_death_protection {
        let death_effects = build_effect_groups(&form.death_effects);
        if form.death_protection || !death_effects.is_empty() {
            if death_effects.is_empty() {
                parts.push("death_protection={}".to_string());
            } else {
                parts.push(format!("death_protection={{death_effects:[{death_effects}]}}"));
            }
        }
    }

    if form.damage_enabled {
        parts.push(format!("damage={}", form.damage.max(0)));
    }
    if form.max_damage_enabled {
        parts.push(format!("max_damage={}", form.max_damage.max(1)));
    }
    if form.stack_enabled {
        parts.push(format!("max_stack_size={}", form.max_stack_size.max(1)));
    }
    if form.repair_enabled {
        parts.push(format!("repair_cost={}", form.repair_cost.max(0)));
    }

    if profile.supports_tooltip_display {
        let hidden = split_csv_str(&form.hidden_components);
        if !hidden.is_empty() {
            let items: Vec<String> = hidden.iter().map(|v| quote(&namespaced(v))).collect();
            parts.push(format!("tooltip_display={{hidden_components:[{}]}}", items.join(",")));
        }
    }

    if form.food_enabled {
        let mut fields = vec![
            format!("nutrition:{}", form.nutrition.max(0)),
            format!("saturation:{}", fmt_number_f64(form.saturation)),
        ];
        if form.always_eat != "默认" {
            fields.push(format!("can_always_eat:{}", if form.always_eat == "是" { "1b" } else { "0b" }));
        }
        parts.push(format!("food={{{}}}", fields.join(",")));
    }

    if profile.supports_consumable {
        let consume_effects = build_effect_groups(&form.consume_effects);
        if form.consumable_enabled || !consume_effects.is_empty() {
            let mut fields: Vec<String> = Vec::new();
            if form.consumable_enabled {
                fields.push(format!("consume_seconds:{}", fmt_number_f64(form.consume_seconds)));
                if !form.consume_sound.trim().is_empty() {
                    fields.push(format!("sound:{}", quote(&namespaced(form.consume_sound.trim()))));
                }
                if form.consume_particles != "默认" {
                    fields.push(format!("has_consume_particles:{}", if form.consume_particles == "是" { "1b" } else { "0b" }));
                }
            }
            if !consume_effects.is_empty() {
                fields.push(format!("on_consume_effects:[{consume_effects}]"));
            }
            parts.push(format!("consumable={{{}}}", fields.join(",")));
        }
    }

    let tool_rules = build_tool_rules(&form.tool_rules);
    if form.tool_enabled || !tool_rules.is_empty() {
        let mut fields: Vec<String> = Vec::new();
        if form.tool_enabled {
            fields.push(format!("default_mining_speed:{}", fmt_number_f64(form.default_mining_speed)));
            fields.push(format!("damage_per_block:{}", form.damage_per_block.max(0)));
        }
        if !tool_rules.is_empty() {
            fields.push(format!("rules:[{tool_rules}]"));
        }
        parts.push(format!("tool={{{}}}", fields.join(",")));
    }

    let body = if parts.is_empty() { String::new() } else { format!("[{}]", parts.join(",")) };
    let slash = if form.with_slash { "/" } else { "" };
    format!(
        "{slash}give {} {}{} {}",
        normalize_target(&form.target),
        map_catalog(ITEMS, &form.item),
        body,
        form.count.max(1)
    )
}

fn build_java121_legacy(form: &GiveForm, warnings: &mut Vec<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let tp = resolve_text_profile(form.version);

    if let Some(line) = form.display_name.first() {
        parts.push(format!("custom_name={}", serialize_text(line, true, &tp, warnings)));
    }
    if let Some(line) = form.item_name.first() {
        parts.push(format!("item_name={}", serialize_text(line, true, &tp, warnings)));
    }
    if !form.lore.is_empty() {
        let items: Vec<String> = form.lore.iter().map(|line| serialize_text(line, true, &tp, warnings)).collect();
        parts.push(format!("lore=[{}]", items.join(",")));
    }

    let rarity = pair_value(RARITIES, &form.rarity);
    if rarity != "none" {
        parts.push(format!("rarity={rarity}"));
    }

    if form.glint != "默认" {
        parts.push(format!("enchantment_glint_override={}", if form.glint == "开启" { "true" } else { "false" }));
    }

    let enchants: Vec<String> = form
        .enchantments
        .iter()
        .filter(|r| !r.id.trim().is_empty())
        .map(|r| format!("{}:{}", component_id(&map_catalog(ENCHANTS, &r.id)), normalize_int(&r.level, 1, 1)))
        .collect();
    if !enchants.is_empty() {
        let inner = format!("{{{}}}", enchants.join(","));
        parts.push(format!("enchantments={{levels:{inner}}}"));
    }

    let attributes: Vec<String> = form
        .attributes
        .iter()
        .filter(|r| !r.r#type.trim().is_empty())
        .map(|r| {
            let mut fields = vec![
                format!("type:{}", quote(&legacy_attribute_type(&r.r#type))),
                format!("amount:{}", fmt_number(&r.amount)),
            ];
            let slot_input = if r.slot.trim().is_empty() { "any" } else { r.slot.as_str() };
            let slot = pair_value(SLOTS, slot_input);
            if !slot.is_empty() && slot != "any" {
                fields.push(format!("slot:{slot}"));
            }
            let id_val = if r.id.trim().is_empty() { crypto_id() } else { r.id.clone() };
            fields.push(format!("id:{}", quote(&id_val)));
            let op_input = if r.operation.trim().is_empty() { "add_value" } else { r.operation.as_str() };
            fields.push(format!("operation:{}", pair_value(OPERATIONS, op_input)));
            format!("{{{}}}", fields.join(","))
        })
        .collect();
    if !attributes.is_empty() {
        parts.push(format!("attribute_modifiers={{modifiers:[{}]}}", attributes.join(",")));
    }

    let place: Vec<&BlockLimitRow> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "place" | "both"))
        .collect();
    let brk: Vec<&BlockLimitRow> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "break" | "both"))
        .collect();
    let place_predicates = block_predicate_list(&place);
    let break_predicates = block_predicate_list(&brk);
    if !place_predicates.is_empty() {
        parts.push(format!("can_place_on={{predicates:[{}]}}", place_predicates.join(",")));
    }
    if !break_predicates.is_empty() {
        parts.push(format!("can_break={{predicates:[{}]}}", break_predicates.join(",")));
    }

    if form.unbreakable {
        parts.push("unbreakable={}".to_string());
    }
    if !form.custom_data.trim().is_empty() {
        parts.push(format!("custom_data={}", form.custom_data.trim()));
    }

    if form.damage_enabled {
        parts.push(format!("damage={}", form.damage.max(0)));
    }
    if form.max_damage_enabled {
        parts.push(format!("max_damage={}", form.max_damage.max(1)));
    }
    if form.stack_enabled {
        parts.push(format!("max_stack_size={}", form.max_stack_size.max(1)));
    }
    if form.repair_enabled {
        parts.push(format!("repair_cost={}", form.repair_cost.max(0)));
    }

    let legacy_food = build_java121_food(form);
    if !legacy_food.is_empty() {
        parts.push(format!("food={legacy_food}"));
    }

    let tool_rules = build_tool_rules(&form.tool_rules);
    if form.tool_enabled || !tool_rules.is_empty() {
        let mut fields: Vec<String> = Vec::new();
        if form.tool_enabled {
            fields.push(format!("default_mining_speed:{}", fmt_number_f64(form.default_mining_speed)));
            fields.push(format!("damage_per_block:{}", form.damage_per_block.max(0)));
        }
        if !tool_rules.is_empty() {
            fields.push(format!("rules:[{tool_rules}]"));
        }
        parts.push(format!("tool={{{}}}", fields.join(",")));
    }

    let body = if parts.is_empty() { String::new() } else { format!("[{}]", parts.join(",")) };
    let slash = if form.with_slash { "/" } else { "" };
    format!(
        "{slash}give {} {}{} {}",
        normalize_target(&form.target),
        map_catalog(ITEMS, &form.item),
        body,
        form.count.max(1)
    )
}

/// 基岩版走自己的 ID 表，不能复用 Java 的 ITEMS/BLOCKS——两版 ID 体系不通用
/// （见 `catalog.rs` 同名注释与蜘蛛网 cobweb/web 之类的实测案例）。
fn build_bedrock(form: &GiveForm) -> String {
    let mut comps: Vec<(&'static str, Json)> = Vec::new();

    let place: Vec<String> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "place" | "both") && !r.block.trim().is_empty())
        .map(|r| component_id(&map_catalog(BEDROCK_BLOCKS, &r.block)))
        .collect();
    let brk: Vec<String> = form
        .block_limits
        .iter()
        .filter(|r| matches!(pair_value(LIMIT_TYPES, &r.r#type).as_str(), "break" | "both") && !r.block.trim().is_empty())
        .map(|r| component_id(&map_catalog(BEDROCK_BLOCKS, &r.block)))
        .collect();

    if !place.is_empty() {
        comps.push((
            "minecraft:can_place_on",
            Json::Obj(vec![("blocks", Json::Arr(place.into_iter().map(Json::Str).collect()))]),
        ));
    }
    if !brk.is_empty() {
        comps.push((
            "minecraft:can_destroy",
            Json::Obj(vec![("blocks", Json::Arr(brk.into_iter().map(Json::Str).collect()))]),
        ));
    }

    let lock_mode = pair_value(ITEM_LOCK_MODES, &form.bedrock_item_lock);
    if lock_mode != "none" {
        comps.push(("minecraft:item_lock", Json::Obj(vec![("mode", Json::Str(lock_mode))])));
    }
    if form.bedrock_keep_on_death {
        comps.push(("minecraft:keep_on_death", Json::Obj(Vec::new())));
    }

    let suffix = if comps.is_empty() { String::new() } else { format!(" {}", Json::Obj(comps).stringify()) };
    let slash = if form.with_slash { "/" } else { "" };
    format!(
        "{slash}give {} {} {} {}{suffix}",
        normalize_target(&form.target),
        component_id(&map_catalog(BEDROCK_ITEMS, &form.item)),
        form.count.max(1),
        form.bedrock_data_value.max(0),
    )
}

fn build_java121_food(form: &GiveForm) -> String {
    let food_effects = build_java121_food_effects(&form.consume_effects);
    if !form.food_enabled && !form.consumable_enabled && food_effects.is_empty() {
        return String::new();
    }
    let mut fields = vec![
        format!("nutrition:{}", form.nutrition.max(0)),
        format!("saturation:{}", fmt_number_f64(form.saturation)),
    ];
    if form.always_eat != "默认" {
        fields.push(format!("can_always_eat:{}", if form.always_eat == "是" { "1b" } else { "0b" }));
    }
    if form.consumable_enabled {
        fields.push(format!("eat_seconds:{}", fmt_number_f64(form.consume_seconds)));
    }
    if !food_effects.is_empty() {
        fields.push(format!("effects:[{food_effects}]"));
    }
    format!("{{{}}}", fields.join(","))
}

fn build_java121_food_effects(groups: &[EffectGroup]) -> String {
    let mut out: Vec<String> = Vec::new();
    for group in groups {
        if group.r#type != "apply_effects" {
            continue;
        }
        let probability = format!("{}f", percent_to_probability(&group.probability_percent));
        for effect in &group.effects {
            let (id_raw, extra) = match effect {
                EffectEntry::Id(s) => (s.clone(), None),
                EffectEntry::Full { id, duration, amplifier, show_particles, show_icon } => {
                    (id.clone(), Some((duration, amplifier, show_particles, show_icon)))
                }
            };
            let id = component_id(&id_raw);
            if id.is_empty() {
                continue;
            }
            let mut fields = vec![format!("id:{id}")];
            if let Some((duration, amplifier, show_particles, show_icon)) = extra {
                fields.push(format!("duration:{}", normalize_int(duration, 0, 0)));
                fields.push(format!("amplifier:{}", normalize_int(amplifier, 0, 0)));
                fields.push(format!("ShowParticles:{}", bool_byte(show_particles.unwrap_or(true))));
                fields.push(format!("ShowIcon:{}", bool_byte(show_icon.unwrap_or(true))));
            }
            let inner = fields.join(",");
            out.push(format!("{{effect:{{{inner}}},probability:{probability}}}"));
        }
    }
    out.join(",")
}

fn build_effect_groups(groups: &[EffectGroup]) -> String {
    let mut out: Vec<String> = Vec::new();
    for group in groups {
        match group.r#type.as_str() {
            "apply_effects" => {
                let effects: Vec<String> = group
                    .effects
                    .iter()
                    .filter_map(|e| {
                        if let EffectEntry::Full { id, duration, amplifier, show_particles, show_icon } = e {
                            let id = component_id(id);
                            if id.is_empty() {
                                return None;
                            }
                            let duration = normalize_int(duration, 0, 0);
                            let amplifier = normalize_int(amplifier, 0, 0);
                            let particles = bool_byte(show_particles.unwrap_or(true));
                            let icon = bool_byte(show_icon.unwrap_or(true));
                            Some(format!(
                                "{{id:{id},duration:{duration},amplifier:{amplifier},ShowParticles:{particles},ShowIcon:{icon}}}"
                            ))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !effects.is_empty() {
                    out.push(format!(
                        "{{type:apply_effects,probability:{},effects:[{}]}}",
                        percent_to_probability(&group.probability_percent),
                        effects.join(",")
                    ));
                }
            }
            "remove_effects" => {
                let effects: Vec<String> = group
                    .effects
                    .iter()
                    .filter_map(|e| {
                        let raw = match e {
                            EffectEntry::Id(s) => s.clone(),
                            EffectEntry::Full { id, .. } => id.clone(),
                        };
                        let id = component_id(&raw);
                        if id.is_empty() { None } else { Some(id) }
                    })
                    .collect();
                if !effects.is_empty() {
                    out.push(format!("{{type:remove_effects,effects:[{}]}}", effects.join(",")));
                }
            }
            "clear_all_effects" => out.push("{type:clear_all_effects}".to_string()),
            "teleport_randomly" => {
                let diameter = if group.diameter.is_null() { Value::from(16.0) } else { group.diameter.clone() };
                out.push(format!("{{type:teleport_randomly,diameter:{}}}", fmt_number(&diameter)));
            }
            _ => {}
        }
    }
    out.join(",")
}

fn build_tool_rules(rules: &[ToolRuleRow]) -> String {
    let mut out: Vec<String> = Vec::new();
    for rule in rules {
        let raw_blocks: Vec<String> = match &rule.blocks {
            Value::Array(arr) => arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            Value::String(s) => split_csv_str(s),
            _ => Vec::new(),
        };
        let blocks: Vec<String> = raw_blocks
            .iter()
            .map(|b| component_id(&map_catalog(BLOCKS, b)))
            .filter(|s| !s.is_empty())
            .collect();
        if blocks.is_empty() {
            continue;
        }
        let mut fields = vec![format!("blocks:[{}]", blocks.join(","))];
        if !value_as_text(Some(&rule.speed)).trim().is_empty() {
            fields.push(format!("speed:{}f", fmt_number(&rule.speed)));
        }
        let correct = pair_value(CORRECT_FOR_DROPS, &rule.correct_for_drops);
        if correct == "true" {
            fields.push("correct_for_drops:1b".to_string());
        }
        if correct == "false" {
            fields.push("correct_for_drops:0b".to_string());
        }
        out.push(format!("{{{}}}", fields.join(",")));
    }
    out.join(",")
}

fn block_predicate_list(rows: &[&BlockLimitRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.block.trim().is_empty())
        .map(|r| format!("{{blocks:{}}}", quote(&map_catalog(BLOCKS, &r.block))))
        .collect()
}

// =====================================================================================
// 有序 JSON 构建（自实现，不用 serde_json::Value/Map——默认不开 preserve_order 特性时
// serde_json 的对象按 key 字母序排列，会打乱 JSON.stringify 依赖插入顺序的输出，
// 那样很多这里要求"逐字符"匹配的测试会假性失败）
// =====================================================================================

#[derive(Debug, Clone)]
enum Json {
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl Json {
    fn stringify(&self) -> String {
        match self {
            Json::Bool(b) => b.to_string(),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Json::Str(s) => serde_json::to_string(s).unwrap(),
            Json::Arr(items) => format!("[{}]", items.iter().map(Json::stringify).collect::<Vec<_>>().join(",")),
            Json::Obj(pairs) => format!(
                "{{{}}}",
                pairs
                    .iter()
                    .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v.stringify()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

fn value_leaf_to_json(v: &Value) -> Json {
    match v {
        Value::Bool(b) => Json::Bool(*b),
        Value::Number(n) => Json::Num(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => Json::Str(s.clone()),
        _ => Json::Str(String::new()),
    }
}

/// JS 的"truthy"判断：`null`/`false`/`0`/`""` 及其等价物为假，其余（含空数组/空对象）为真。
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// 近似 JS `Number(value)`：数字原样、字符串按数值解析（空串/纯空白 -> 0）、
/// 布尔 true/false -> 1/0、null -> 0，其余类型返回 None（视作 NaN）。
fn js_number_of(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() { Some(0.0) } else { t.parse::<f64>().ok() }
        }
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        _ => None,
    }
}

/// 近似 JS `String(value ?? "")`。
fn value_as_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|f| if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", f as i64) } else { format!("{f}") })
            .unwrap_or_default(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn normalize_int(value: &Value, fallback: i64, min: i64) -> i64 {
    match js_number_of(value) {
        Some(n) if n.is_finite() => (n.floor() as i64).max(min),
        _ => fallback,
    }
}

fn normalize_number(value: &Value, fallback: f64, min: f64) -> f64 {
    match js_number_of(value) {
        Some(n) if n.is_finite() => n.max(min),
        _ => fallback,
    }
}

/// 对应客户端 `fmtNumber`：整数原样输出，小数按 `toFixed(10)` 再去掉多余的尾零/小数点。
pub fn fmt_number(value: &Value) -> String {
    match js_number_of(value) {
        Some(n) if n.is_finite() => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", n as i64)
            } else {
                let s = format!("{n:.10}");
                let s = s.trim_end_matches('0');
                let s = s.trim_end_matches('.');
                s.to_string()
            }
        }
        _ => value_as_text(Some(value)).trim().to_string(),
    }
}

/// `fmt_number` 的 f64 直传版本，方便 GiveForm 里已经是 f64 的字段调用。
pub fn fmt_number_f64(value: f64) -> String {
    fmt_number(&Value::from(value))
}

/// 对应客户端 `percentToProbability`：百分比（0~100）clamp 后转成 0~1 小数字符串。
pub fn percent_to_probability(value: &Value) -> String {
    let n = js_number_of(value).filter(|n| n.is_finite()).unwrap_or(0.0);
    let clamped = n.max(0.0).min(100.0);
    fmt_number(&Value::from(clamped / 100.0))
}

/// 对应客户端 `boolByte`。
pub fn bool_byte(value: bool) -> &'static str {
    if value { "1b" } else { "0b" }
}

fn split_csv_str(s: &str) -> Vec<String> {
    s.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect()
}

/// 对应客户端 `splitCsv(value: string | string[])`。
pub fn split_csv(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Value::String(s) => split_csv_str(s),
        _ => split_csv_str(&value_as_text(Some(value))),
    }
}

/// 对应客户端 `quote`：`JSON.stringify(string)`。
pub fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

/// 对应客户端 `snbtJsonString`：把已序列化的 JSON 文本包成 SNBT 单引号字符串。
pub fn snbt_json_string(json: &str) -> String {
    format!("'{}'", json.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// 对应客户端 `stripMinecraftNamespace`。
pub fn strip_minecraft_namespace(value: &str) -> String {
    let text = value.trim();
    match text.strip_prefix("minecraft:") {
        Some(rest) => rest.to_string(),
        None => text.to_string(),
    }
}

/// 对应客户端 `componentId`：先补/保留命名空间，再把 `minecraft:` 前缀剥掉
/// （很多组件字段——附魔 id、属性 id、效果 id——按 Mojang 惯例是不带命名空间的裸 id）。
pub fn component_id(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    strip_minecraft_namespace(&namespaced(text))
}

fn legacy_attribute_type(value: &str) -> String {
    let id = component_id(&map_catalog(ATTRIBUTES, value));
    if id.contains('.') {
        return id;
    }
    const MAPPED: &[(&str, &str)] = &[
        ("armor", "generic.armor"),
        ("armor_toughness", "generic.armor_toughness"),
        ("attack_damage", "generic.attack_damage"),
        ("attack_knockback", "generic.attack_knockback"),
        ("attack_speed", "generic.attack_speed"),
        ("block_break_speed", "generic.block_break_speed"),
        ("block_interaction_range", "player.block_interaction_range"),
        ("entity_interaction_range", "player.entity_interaction_range"),
        ("fall_damage_multiplier", "generic.fall_damage_multiplier"),
        ("knockback_resistance", "generic.knockback_resistance"),
        ("luck", "generic.luck"),
        ("max_absorption", "generic.max_absorption"),
        ("max_health", "generic.max_health"),
        ("mining_efficiency", "player.mining_efficiency"),
        ("oxygen_bonus", "generic.oxygen_bonus"),
        ("safe_fall_distance", "generic.safe_fall_distance"),
        ("sneaking_speed", "player.sneaking_speed"),
        ("submerged_mining_speed", "player.submerged_mining_speed"),
        ("water_movement_efficiency", "generic.water_movement_efficiency"),
    ];
    for (k, v) in MAPPED {
        if *k == id {
            return v.to_string();
        }
    }
    format!("generic.{id}")
}

fn normalize_target(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() { "@a".to_string() } else { text.to_string() }
}

fn crypto_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:x}-{n:x}")
}

// ---------------- 小型 label<->value 对照表（不是自动生成 catalog，手写维护） ----------------
// 对应 `src/data/catalog.ts` 里的同名 PairRow 常量。这些是 UI 下拉框用的中文标签<->
// 内部值映射，条目很少，不走 gen-catalog 脚本，改版本时如果这些表变了要手动同步。

const SLOTS: &[(&str, &str)] = &[
    ("任意", "any"),
    ("手部", "hand"),
    ("盔甲", "armor"),
    ("身体", "body"),
    ("主手", "mainhand"),
    ("副手", "offhand"),
    ("头部", "head"),
    ("胸部", "chest"),
    ("腿部", "legs"),
    ("脚部", "feet"),
];

const OPERATIONS: &[(&str, &str)] = &[
    ("加算", "add_value"),
    ("基值乘算", "add_multiplied_base"),
    ("总值乘算", "add_multiplied_total"),
];

const RARITIES: &[(&str, &str)] = &[
    ("不设置", "none"),
    ("普通", "common"),
    ("罕见", "uncommon"),
    ("稀有", "rare"),
    ("史诗", "epic"),
];

const LIMIT_TYPES: &[(&str, &str)] = &[("可放置", "place"), ("可破坏", "break"), ("两者", "both")];

const ITEM_LOCK_MODES: &[(&str, &str)] = &[
    ("不设置", "none"),
    ("锁定背包", "lock_in_inventory"),
    ("锁定槽位", "lock_in_slot"),
];

const CORRECT_FOR_DROPS: &[(&str, &str)] = &[("默认", "default"), ("是", "true"), ("否", "false")];

/// 对应客户端 `pairValue`：按中文标签或内部值反查内部值，两者都不匹配就原样返回
/// （通常意味着调用方直接给的就是内部值，如 AI 意图数据）。
fn pair_value(pairs: &[(&str, &str)], text: &str) -> String {
    for (label, value) in pairs {
        if text == *label || text == *value {
            return value.to_string();
        }
    }
    text.to_string()
}

// =====================================================================================
// 文本组件序列化
// =====================================================================================
// 版本敏感能力（按 Java 版本先后判定，而非按粗粒度 item 组件 profile）：
//   object 组件      -> 1.21.9+
//   click/hover 新式 -> 1.21.5+（否则 camelCase 旧式）
//   shadow_color 数组 -> 1.21.4+（否则打包整数）

const JAVA_VERSION_ORDER: &[GiveVersion] = &[
    GiveVersion::Java1_20_5,
    GiveVersion::Java1_21,
    GiveVersion::Java1_21_1,
    GiveVersion::Java1_21_2,
    GiveVersion::Java1_21_3,
    GiveVersion::Java1_21_4,
    GiveVersion::Java1_21_5,
    GiveVersion::Java1_21_6,
    GiveVersion::Java1_21_9,
    GiveVersion::Java1_21_11Plus,
    GiveVersion::Java26_1,
    GiveVersion::Java26_2Plus,
];

fn version_at_least(version: GiveVersion, min: GiveVersion) -> bool {
    let iv = JAVA_VERSION_ORDER.iter().position(|v| *v == version);
    let im = JAVA_VERSION_ORDER.iter().position(|v| *v == min);
    matches!((iv, im), (Some(a), Some(b)) if a >= b)
}

pub struct TextProfile {
    pub supports_object_component: bool,
    pub event_format_modern: bool,
    pub supports_shadow_array: bool,
}

/// 对应客户端 `resolveTextProfile`。
pub fn resolve_text_profile(version: GiveVersion) -> TextProfile {
    TextProfile {
        supports_object_component: version_at_least(version, GiveVersion::Java1_21_9),
        event_format_modern: version_at_least(version, GiveVersion::Java1_21_5),
        supports_shadow_array: version_at_least(version, GiveVersion::Java1_21_4),
    }
}

fn normalize_shadow(shadow: &Value, tp: &TextProfile) -> Json {
    if let Some(arr) = shadow.as_array() {
        if tp.supports_shadow_array {
            return Json::Arr(arr.iter().map(value_leaf_to_json).collect());
        }
        let get = |i: usize, default: f64| arr.get(i).and_then(js_number_of).unwrap_or(default);
        let r = get(0, 0.0);
        let g = get(1, 0.0);
        let b = get(2, 0.0);
        let a = get(3, 1.0);
        let rr = ((r * 255.0).round() as i64) & 0xff;
        let gg = ((g * 255.0).round() as i64) & 0xff;
        let bb = ((b * 255.0).round() as i64) & 0xff;
        let aa = ((a * 255.0).round() as i64) & 0xff;
        let value = (aa << 24) | (rr << 16) | (gg << 8) | bb;
        let value = if value >= 2i64.pow(31) { value - 2i64.pow(32) } else { value };
        Json::Num(value as f64)
    } else {
        value_leaf_to_json(shadow)
    }
}

fn shape_click_event(ev: &Value, tp: &TextProfile) -> Option<(&'static str, Json)> {
    let action = ev.get("action").and_then(Value::as_str).filter(|a| !a.is_empty())?;
    let val = ev.get("value").and_then(Value::as_str).unwrap_or("");
    if tp.event_format_modern {
        let mut out = vec![("action", Json::Str(action.to_string()))];
        match action {
            "open_url" => out.push(("url", Json::Str(val.to_string()))),
            "run_command" | "suggest_command" => out.push(("command", Json::Str(val.to_string()))),
            "copy_to_clipboard" => out.push(("value", Json::Str(val.to_string()))),
            "change_page" => out.push(("page", Json::Num(normalize_int(&Value::String(val.to_string()), 1, 1) as f64))),
            "show_dialog" => out.push(("dialog", Json::Str(val.to_string()))),
            _ => {}
        }
        return Some(("click_event", Json::Obj(out)));
    }
    // 旧式 clickEvent{action,value}（show_dialog 为 1.21.6+，旧版不支持则丢弃）
    if action == "show_dialog" {
        return None;
    }
    let value = if action == "change_page" {
        normalize_int(&Value::String(val.to_string()), 1, 1).to_string()
    } else {
        val.to_string()
    };
    Some(("clickEvent", Json::Obj(vec![("action", Json::Str(action.to_string())), ("value", Json::Str(value))])))
}

fn shape_hover_event(ev: &Value, tp: &TextProfile, warnings: &mut Vec<String>) -> Option<(&'static str, Json)> {
    let action = ev.get("action").and_then(Value::as_str).filter(|a| !a.is_empty())?;
    if tp.event_format_modern {
        let mut out = vec![("action", Json::Str(action.to_string()))];
        match action {
            "show_text" => {
                let val = match ev.get("text") {
                    Some(t) if truthy(t) => json_rich_line_value(t, tp, warnings),
                    _ => Json::Str(String::new()),
                };
                out.push(("value", val));
            }
            "show_item" => {
                let id = ev.get("itemId").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("stone");
                out.push(("id", Json::Str(namespaced(id))));
                if let Some(c) = ev.get("itemCount") {
                    if !c.is_null() {
                        out.push(("count", value_leaf_to_json(c)));
                    }
                }
            }
            "show_entity" => {
                let et = ev.get("entityType").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("pig");
                out.push(("id", Json::Str(namespaced(et))));
                if let Some(u) = ev.get("entityUuid").and_then(Value::as_str) {
                    if !u.is_empty() {
                        out.push(("uuid", Json::Str(u.to_string())));
                    }
                }
                if let Some(n) = ev.get("entityName") {
                    if truthy(n) {
                        out.push(("name", json_rich_line_value(n, tp, warnings)));
                    }
                }
            }
            _ => {}
        }
        return Some(("hover_event", Json::Obj(out)));
    }
    // 旧式 hoverEvent{action,contents:{...}}
    let mut out = vec![("action", Json::Str(action.to_string()))];
    match action {
        "show_text" => {
            let val = match ev.get("text") {
                Some(t) if truthy(t) => json_rich_line_value(t, tp, warnings),
                _ => Json::Str(String::new()),
            };
            out.push(("contents", val));
        }
        "show_item" => {
            let id = ev.get("itemId").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("stone");
            let mut contents = vec![("id", Json::Str(namespaced(id)))];
            if let Some(c) = ev.get("itemCount") {
                if !c.is_null() {
                    contents.push(("count", value_leaf_to_json(c)));
                }
            }
            out.push(("contents", Json::Obj(contents)));
        }
        "show_entity" => {
            let et = ev.get("entityType").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("pig");
            let mut contents = vec![("type", Json::Str(namespaced(et)))];
            if let Some(u) = ev.get("entityUuid").and_then(Value::as_str) {
                if !u.is_empty() {
                    contents.push(("id", Json::Str(u.to_string())));
                }
            }
            if let Some(n) = ev.get("entityName") {
                if truthy(n) {
                    contents.push(("name", json_rich_line_value(n, tp, warnings)));
                }
            }
            out.push(("contents", Json::Obj(contents)));
        }
        _ => {}
    }
    Some(("hoverEvent", Json::Obj(out)))
}

fn apply_style(out: &mut Vec<(&'static str, Json)>, run: &Value, tp: &TextProfile, warnings: &mut Vec<String>) {
    if let Some(v) = run.get("bold").and_then(Value::as_bool) {
        out.push(("bold", Json::Bool(v)));
    }
    if let Some(v) = run.get("italic").and_then(Value::as_bool) {
        out.push(("italic", Json::Bool(v)));
    }
    if let Some(v) = run.get("underlined").and_then(Value::as_bool) {
        out.push(("underlined", Json::Bool(v)));
    }
    if let Some(v) = run.get("strikethrough").and_then(Value::as_bool) {
        out.push(("strikethrough", Json::Bool(v)));
    }
    if let Some(v) = run.get("obfuscated").and_then(Value::as_bool) {
        out.push(("obfuscated", Json::Bool(v)));
    }
    if let Some(v) = run.get("color").and_then(Value::as_str) {
        out.push(("color", Json::Str(v.to_string())));
    }
    if let Some(v) = run.get("font").and_then(Value::as_str) {
        out.push(("font", Json::Str(v.to_string())));
    }
    if let Some(v) = run.get("shadow_color") {
        if !v.is_null() {
            out.push(("shadow_color", normalize_shadow(v, tp)));
        }
    }
    if let Some(v) = run.get("insertion").and_then(Value::as_str) {
        out.push(("insertion", Json::Str(v.to_string())));
    }
    if let Some(ce) = run.get("click_event") {
        if truthy(ce) {
            if let Some((key, val)) = shape_click_event(ce, tp) {
                out.push((key, val));
            }
        }
    }
    if let Some(he) = run.get("hover_event") {
        if truthy(he) {
            if let Some((key, val)) = shape_hover_event(he, tp, warnings) {
                out.push((key, val));
            }
        }
    }
}

/// 把一个运行整形为纯 JSON 对象（键名精确、按版本门控）。不合法/不支持返回 None（剥离）。
fn component_to_json(run: &Value, tp: &TextProfile, warnings: &mut Vec<String>) -> Option<Json> {
    if !run.is_object() {
        return None;
    }
    let type_ = run.get("type").and_then(Value::as_str).unwrap_or("text");
    let mut out: Vec<(&'static str, Json)> = Vec::new();

    match type_ {
        "translatable" => {
            out.push(("type", Json::Str("translatable".to_string())));
            out.push(("translate", Json::Str(run.get("translate").and_then(Value::as_str).unwrap_or("").to_string())));
            if let Some(fb) = run.get("fallback") {
                if !fb.is_null() {
                    out.push(("fallback", value_leaf_to_json(fb)));
                }
            }
            if let Some(with) = run.get("with").and_then(Value::as_array) {
                if !with.is_empty() {
                    let mapped: Vec<Json> = with.iter().filter_map(|c| component_to_json(c, tp, warnings)).collect();
                    out.push(("with", Json::Arr(mapped)));
                }
            }
        }
        "object" => {
            if !tp.supports_object_component {
                warnings.push("内嵌图标/头像（object 组件）需要 Java 1.21.9+，已忽略".to_string());
                return None;
            }
            out.push(("type", Json::Str("object".to_string())));
            if run.get("object").and_then(Value::as_str) == Some("player") {
                out.push(("object", Json::Str("player".to_string())));
                out.push(("player", Json::Str(run.get("player").and_then(Value::as_str).unwrap_or("").to_string())));
                if let Some(h) = run.get("hat").and_then(Value::as_bool) {
                    out.push(("hat", Json::Bool(h)));
                }
            } else {
                out.push(("object", Json::Str("atlas".to_string())));
                let atlas = run.get("atlas").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("minecraft:blocks");
                out.push(("atlas", Json::Str(atlas.to_string())));
                out.push(("sprite", Json::Str(run.get("sprite").and_then(Value::as_str).unwrap_or("").to_string())));
            }
        }
        "keybind" => {
            out.push(("keybind", Json::Str(run.get("keybind").and_then(Value::as_str).unwrap_or("").to_string())));
        }
        "selector" => {
            out.push(("selector", Json::Str(run.get("selector").and_then(Value::as_str).unwrap_or("").to_string())));
            if let Some(sep) = run.get("separator") {
                if truthy(sep) {
                    if let Some(j) = component_to_json(sep, tp, warnings) {
                        out.push(("separator", j));
                    }
                }
            }
        }
        "score" => {
            let score = run.get("score");
            let name = score.and_then(|s| s.get("name")).and_then(Value::as_str).unwrap_or("");
            let objective = score.and_then(|s| s.get("objective")).and_then(Value::as_str).unwrap_or("");
            out.push(("score", Json::Obj(vec![("name", Json::Str(name.to_string())), ("objective", Json::Str(objective.to_string()))])));
        }
        "nbt" => {
            out.push(("nbt", Json::Str(run.get("nbt").and_then(Value::as_str).unwrap_or("").to_string())));
            let source = run.get("source").and_then(Value::as_str).unwrap_or("block");
            out.push(("source", Json::Str(source.to_string())));
            if source == "block" {
                if let Some(b) = run.get("block").and_then(Value::as_str) {
                    if !b.is_empty() {
                        out.push(("block", Json::Str(b.to_string())));
                    }
                }
            }
            if source == "entity" {
                if let Some(e) = run.get("entity").and_then(Value::as_str) {
                    if !e.is_empty() {
                        out.push(("entity", Json::Str(e.to_string())));
                    }
                }
            }
            if source == "storage" {
                if let Some(s) = run.get("storage").and_then(Value::as_str) {
                    if !s.is_empty() {
                        out.push(("storage", Json::Str(namespaced(s))));
                    }
                }
            }
            if let Some(i) = run.get("interpret").and_then(Value::as_bool) {
                out.push(("interpret", Json::Bool(i)));
            }
            if let Some(sep) = run.get("separator") {
                if truthy(sep) {
                    if let Some(j) = component_to_json(sep, tp, warnings) {
                        out.push(("separator", j));
                    }
                }
            }
        }
        _ => {
            // text（含无 type 的旧模板）
            out.push(("text", Json::Str(value_as_text(run.get("text")))));
        }
    }

    apply_style(&mut out, run, tp, warnings);
    Some(Json::Obj(out))
}

fn json_rich_line_slice(line: &[Value], tp: &TextProfile, warnings: &mut Vec<String>) -> Json {
    Json::Arr(line.iter().filter_map(|c| component_to_json(c, tp, warnings)).collect())
}

fn json_rich_line_value(v: &Value, tp: &TextProfile, warnings: &mut Vec<String>) -> Json {
    match v.as_array() {
        Some(arr) => json_rich_line_slice(arr, tp, warnings),
        None => Json::Arr(Vec::new()),
    }
}

/// 序列化单行文本组件：asSnbt 时包成单引号 SNBT 字符串（early/legacy/mid 族），否则裸 JSON（modern 族）。
fn serialize_text(line: &[Value], as_snbt: bool, tp: &TextProfile, warnings: &mut Vec<String>) -> String {
    let json = json_rich_line_slice(line, tp, warnings).stringify();
    if as_snbt { snbt_json_string(&json) } else { json }
}

/// 把一行富文本序列化为 SNBT 单引号字符串，供实体 CustomName / 方块实体文本使用。
/// 与 /give 的文本组件同源（同一套 json_rich_line + 版本档案），保证两处写法一致。
pub fn rich_line_to_snbt_string(line: &[Value], version: GiveVersion, warnings: &mut Vec<String>) -> String {
    serialize_text(line, true, &resolve_text_profile(version), warnings)
}

// =====================================================================================
// 版本判断族
// =====================================================================================

pub fn is_java121_legacy_family(version: GiveVersion) -> bool {
    matches!(version, GiveVersion::Java1_21 | GiveVersion::Java1_21_1)
}

pub fn is_java1205_family(version: GiveVersion) -> bool {
    version == GiveVersion::Java1_20_5
}

pub fn is_java1212_family(version: GiveVersion) -> bool {
    matches!(version, GiveVersion::Java1_21_2 | GiveVersion::Java1_21_3 | GiveVersion::Java1_21_4)
}

/// 1.21.5+ 现代实体 NBT 族：属性用 attributes[]/id/base（无 generic. 前缀）、
/// 装备用 equipment{} 而非 HandItems[]/ArmorItems[]。基岩版不属于该族。
pub fn is_modern_nbt_family(version: GiveVersion) -> bool {
    version_at_least(version, GiveVersion::Java1_21_5)
}

fn get_modern_profile(version: GiveVersion) -> ModernProfile {
    if version == GiveVersion::Bedrock || is_java121_legacy_family(version) {
        ModernProfile {
            text_as_snbt_string: false,
            adventure_predicate_wrapper: false,
            supports_tooltip_display: false,
            supports_consumable: false,
            supports_glider: false,
            supports_death_protection: false,
            supports_attribute_modifiers: false,
        }
    } else if is_java1205_family(version) {
        JAVA_1_20_5_PROFILE
    } else if is_java1212_family(version) {
        JAVA_1_21_2_PROFILE
    } else {
        MODERN_PROFILE
    }
}

fn normalize_version(text: &str) -> GiveVersion {
    match text {
        "java_1_20_5" => GiveVersion::Java1_20_5,
        "java_1_21" => GiveVersion::Java1_21,
        "java_1_21_1" => GiveVersion::Java1_21_1,
        "java_1_21_2" => GiveVersion::Java1_21_2,
        "java_1_21_3" => GiveVersion::Java1_21_3,
        "java_1_21_4" => GiveVersion::Java1_21_4,
        "java_1_21_5" => GiveVersion::Java1_21_5,
        "java_1_21_6" => GiveVersion::Java1_21_6,
        "java_1_21_9" => GiveVersion::Java1_21_9,
        "java_1_21_11_plus" => GiveVersion::Java1_21_11Plus,
        "java_26_1" => GiveVersion::Java26_1,
        "java_26_2_plus" => GiveVersion::Java26_2Plus,
        "bedrock" => GiveVersion::Bedrock,
        _ => GiveVersion::Java1_21_11Plus,
    }
}

/// 把存档 level.dat 读到的原始版本字符串（如 "1.21.7"、"26.2"）映射到 GiveVersion 分档，
/// 供"识别到的存档版本和当前选择不一致"这类提示用（部署面板）。
///
/// 边界要和 `src/data/catalog.ts` 的 VERSIONS 表（下拉框展示的"Java 1.21.6~1.21.8"之类
/// 范围文案）保持一致——两边各自维护而不是互相解析对方格式。识别不出来（低于支持范围、
/// 格式不对）返回 `None`，调用方应该当成"识别不到，不打扰用户"处理。
pub fn detect_give_version_from_raw(raw: &str) -> Option<GiveVersion> {
    let trimmed = raw.trim();
    let parts: Vec<i64> = trimmed.split('.').map(|p| p.parse::<i64>()).collect::<Result<Vec<_>, _>>().ok()?;
    if parts.is_empty() {
        return None;
    }

    // 新计年法（26.x 起）没有前导 "1."，直接按 major.minor 比较。
    if parts[0] >= 2 {
        let major = parts[0];
        let minor = parts.get(1).copied().unwrap_or(0);
        if major == 26 && minor == 1 {
            return Some(GiveVersion::Java26_1);
        }
        if major >= 26 && minor >= 2 {
            return Some(GiveVersion::Java26_2Plus);
        }
        if major > 26 {
            return Some(GiveVersion::Java26_2Plus); // 更新的年份先沿用最新分档
        }
        return None;
    }

    // 老计年法：1.x.y
    if parts[0] != 1 {
        return None;
    }
    let minor = *parts.get(1)?;
    let patch = parts.get(2).copied().unwrap_or(0);
    if minor == 20 {
        return if patch >= 5 { Some(GiveVersion::Java1_20_5) } else { None };
    }
    if minor != 21 {
        return None;
    }
    match patch {
        0 => Some(GiveVersion::Java1_21),
        1 => Some(GiveVersion::Java1_21_1),
        2 => Some(GiveVersion::Java1_21_2),
        3 => Some(GiveVersion::Java1_21_3),
        4 => Some(GiveVersion::Java1_21_4),
        5 => Some(GiveVersion::Java1_21_5),
        6..=8 => Some(GiveVersion::Java1_21_6),
        9..=10 => Some(GiveVersion::Java1_21_9),
        _ => Some(GiveVersion::Java1_21_11Plus), // patch >= 11
    }
}

// =====================================================================================
// 颜色渐变工具（lore/displayName 的颜色渐变生成用）
// =====================================================================================

/// 对应客户端 `hexToRgb`。
pub fn hex_to_rgb(value: &str) -> Result<(u8, u8, u8), String> {
    let text = value.trim();
    let valid = text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err("颜色必须是 #RRGGBB".to_string());
    }
    let r = u8::from_str_radix(&text[1..3], 16).map_err(|_| "颜色必须是 #RRGGBB".to_string())?;
    let g = u8::from_str_radix(&text[3..5], 16).map_err(|_| "颜色必须是 #RRGGBB".to_string())?;
    let b = u8::from_str_radix(&text[5..7], 16).map_err(|_| "颜色必须是 #RRGGBB".to_string())?;
    Ok((r, g, b))
}

/// 对应客户端 `rgbToHex`。
pub fn rgb_to_hex(value: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", value.0, value.1, value.2)
}

/// 对应客户端 `colorLerp`。
pub fn color_lerp(start: &str, end: &str, count: i64) -> Result<Vec<String>, String> {
    if count <= 0 {
        return Ok(Vec::new());
    }
    let a = hex_to_rgb(start)?;
    let b = hex_to_rgb(end)?;
    if count == 1 {
        return Ok(vec![rgb_to_hex(a)]);
    }
    let mut out = Vec::with_capacity(count as usize);
    for index in 0..count {
        let ratio = index as f64 / (count - 1) as f64;
        let r = (a.0 as f64 + (b.0 as f64 - a.0 as f64) * ratio).round() as u8;
        let g = (a.1 as f64 + (b.1 as f64 - a.1 as f64) * ratio).round() as u8;
        let bl = (a.2 as f64 + (b.2 as f64 - a.2 as f64) * ratio).round() as u8;
        out.push(rgb_to_hex((r, g, bl)));
    }
    Ok(out)
}

/// 对应客户端 `shadowColorInt`。
pub fn shadow_color_int(hex_color: &str, alpha_percent: f64) -> Result<i64, String> {
    let (r, g, b) = hex_to_rgb(hex_color)?;
    let alpha = (alpha_percent.max(0.0).min(100.0) / 100.0 * 255.0).round() as i64;
    let value = (alpha << 24) | ((r as i64) << 16) | ((g as i64) << 8) | (b as i64);
    Ok(if value >= 2i64.pow(31) { value - 2i64.pow(32) } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serde_tags_match_ts_string_literals() {
        // 与客户端 GiveVersion 联合类型的字符串字面量逐一核对。
        let cases: &[(GiveVersion, &str)] = &[
            (GiveVersion::Java1_20_5, "\"java_1_20_5\""),
            (GiveVersion::Java1_21, "\"java_1_21\""),
            (GiveVersion::Java1_21_1, "\"java_1_21_1\""),
            (GiveVersion::Java1_21_2, "\"java_1_21_2\""),
            (GiveVersion::Java1_21_3, "\"java_1_21_3\""),
            (GiveVersion::Java1_21_4, "\"java_1_21_4\""),
            (GiveVersion::Java1_21_5, "\"java_1_21_5\""),
            (GiveVersion::Java1_21_6, "\"java_1_21_6\""),
            (GiveVersion::Java1_21_9, "\"java_1_21_9\""),
            (GiveVersion::Java1_21_11Plus, "\"java_1_21_11_plus\""),
            (GiveVersion::Java26_1, "\"java_26_1\""),
            (GiveVersion::Java26_2Plus, "\"java_26_2_plus\""),
            (GiveVersion::Bedrock, "\"bedrock\""),
        ];
        for (version, expected) in cases {
            assert_eq!(serde_json::to_string(version).unwrap(), *expected);
            let parsed: GiveVersion = serde_json::from_str(expected).unwrap();
            assert_eq!(parsed, *version);
        }
    }

    // ---------------- builder.test.mjs 逐条移植（108 条 expect） ----------------
    // 每个 #[test] 对应 TS 源文件里的一个编号区块，注释保留原编号方便对照。

    fn base(version: GiveVersion) -> GiveForm {
        let mut f = create_default_form();
        f.version = version;
        f.item = "minecraft:stone".to_string();
        f.target = "@a".to_string();
        f.count = 1;
        f
    }

    fn build(form: &GiveForm) -> String {
        let mut warnings = Vec::new();
        build_give_command(form, &mut warnings)
    }

    fn build_w(form: &GiveForm) -> (String, Vec<String>) {
        let mut warnings = Vec::new();
        let cmd = build_give_command(form, &mut warnings);
        (cmd, warnings)
    }

    #[test]
    fn t01_java_1_21_11_plus_basic_item() {
        let f = base(GiveVersion::Java1_21_11Plus);
        assert_eq!(build(&f), "give @a minecraft:stone 1");
    }

    #[test]
    fn t02_java_1_21_11_plus_item_name_present() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.item_name = vec![vec![json!({"text": "My Item"})]];
        assert!(build(&f).contains("item_name="));
    }

    #[test]
    fn t03_java_1_21_item_name_was_broken_now_fixed() {
        let mut f = base(GiveVersion::Java1_21);
        f.item_name = vec![vec![json!({"text": "物品名称", "underlined": true, "color": "#000599"})]];
        let cmd = build(&f);
        assert!(cmd.contains("item_name="));
        assert!(cmd.contains("item_name='"));
    }

    #[test]
    fn t04_java_1_21_1_item_name() {
        let mut f = base(GiveVersion::Java1_21_1);
        f.item_name = vec![vec![json!({"text": "名称"})]];
        assert!(build(&f).contains("item_name='"));
    }

    #[test]
    fn t05_java_1_21_custom_name_snbt_format() {
        let mut f = base(GiveVersion::Java1_21);
        f.display_name = vec![vec![json!({"text": "字", "bold": true, "italic": false, "color": "#000599"})]];
        assert!(build(&f).contains("custom_name='"));
    }

    #[test]
    fn t06_java_1_21_enchantments_levels_wrapper() {
        let mut f = base(GiveVersion::Java1_21);
        f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
        assert!(build(&f).contains("enchantments={levels:{unbreaking:3}}"));
    }

    #[test]
    fn t07_java_1_21_attribute_generic_prefix_and_modifiers_wrapper() {
        let mut f = base(GiveVersion::Java1_21);
        f.attributes = vec![AttributeRow {
            r#type: "armor".to_string(),
            amount: json!(10),
            slot: "任意".to_string(),
            operation: "加算".to_string(),
            id: "99".to_string(),
        }];
        let cmd = build(&f);
        assert!(cmd.contains("\"generic.armor\""));
        assert!(cmd.contains("attribute_modifiers={modifiers:["));
    }

    #[test]
    fn t08_java_1_21_11_plus_enchantments_no_levels_wrapper() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
        assert!(build(&f).contains("enchantments={unbreaking:3}"));
    }

    #[test]
    fn t09_java_1_21_food_eat_seconds_merging() {
        let mut f = base(GiveVersion::Java1_21);
        f.food_enabled = true;
        f.nutrition = 5;
        f.saturation = 6.0;
        f.consumable_enabled = true;
        f.consume_seconds = 2.0;
        let cmd = build(&f);
        assert!(cmd.contains("eat_seconds:2"));
        assert!(!cmd.contains("consumable="));
    }

    #[test]
    fn t10_bedrock_basic_format() {
        let f = base(GiveVersion::Bedrock);
        assert!(build(&f).starts_with("give @a stone 1 0"));
    }

    #[test]
    fn t10b_bedrock_uses_its_own_id_table() {
        // 曾经是真 bug：早先 buildBedrock 查的是 Java 的 ITEMS/BLOCKS。
        let cases: &[(&str, &str)] = &[
            ("蜘蛛网", "web"),          // Java: cobweb
            ("枯萎的灌木", "deadbush"), // Java: dead_bush
            ("南瓜灯", "lit_pumpkin"),  // Java: jack_o_lantern
            ("睡莲", "waterlily"),      // Java: lily_pad
            ("音符盒", "noteblock"),    // Java: note_block
            ("末地石砖", "end_bricks"), // Java: end_stone_bricks
        ];
        for (zh, bedrock_id) in cases {
            let mut f = base(GiveVersion::Bedrock);
            f.item = zh.to_string();
            assert_eq!(build(&f), format!("give @a {bedrock_id} 1 0"), "基岩版「{zh}」应使用基岩 id {bedrock_id}");
        }
        // 对照：同样的中文名在 Java 版必须仍然走 Java 的 id
        let mut j = base(GiveVersion::Java1_21_11Plus);
        j.item = "蜘蛛网".to_string();
        assert_eq!(build(&j), "give @a minecraft:cobweb 1");
    }

    #[test]
    fn t10b2_bedrock_block_limits_use_bedrock_ids() {
        let mut f = base(GiveVersion::Bedrock);
        f.block_limits = vec![BlockLimitRow { block: "音符盒".to_string(), r#type: "可放置".to_string() }];
        assert!(build(&f).contains("\"noteblock\""));
    }

    #[test]
    fn t10b3_bedrock_only_item_generates_fine() {
        let mut f = base(GiveVersion::Bedrock);
        f.item = "边界".to_string();
        assert_eq!(build(&f), "give @a border_block 1 0");
    }

    #[test]
    fn t11_java_1_21_lore_is_snbt_string_array() {
        let mut f = base(GiveVersion::Java1_21);
        f.lore = vec![vec![json!({"text": "第一行"})], vec![json!({"text": "第二行"})]];
        let cmd = build(&f);
        assert!(cmd.contains("lore=['") || cmd.contains("lore=[\""));
        assert!(cmd.contains("lore=['"));
    }

    #[test]
    fn t12_java_1_21_unbreakable() {
        let mut f = base(GiveVersion::Java1_21);
        f.unbreakable = true;
        assert!(build(&f).contains("unbreakable={}"));
    }

    #[test]
    fn t13_java_1_21_no_glider_no_death_protection() {
        let mut f = base(GiveVersion::Java1_21);
        f.glider = true;
        f.death_protection = true;
        let cmd = build(&f);
        assert!(!cmd.contains("glider"));
        assert!(!cmd.contains("death_protection"));
    }

    #[test]
    fn t14_java_1_21_can_place_on_can_break_predicates_wrapper() {
        let mut f = base(GiveVersion::Java1_21);
        f.block_limits = vec![
            BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() },
            BlockLimitRow { block: "minecraft:dirt".to_string(), r#type: "break".to_string() },
        ];
        let cmd = build(&f);
        assert!(cmd.contains(r#"can_place_on={predicates:[{blocks:"minecraft:stone"}]}"#));
        assert!(cmd.contains(r#"can_break={predicates:[{blocks:"minecraft:dirt"}]}"#));
    }

    #[test]
    fn t15_java_1_21_11_plus_can_place_on_can_break_direct_list() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.block_limits = vec![
            BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() },
            BlockLimitRow { block: "minecraft:dirt".to_string(), r#type: "break".to_string() },
        ];
        let cmd = build(&f);
        assert!(cmd.contains(r#"can_place_on=[{blocks:"minecraft:stone"}]"#));
        assert!(cmd.contains(r#"can_break=[{blocks:"minecraft:dirt"}]"#));
        assert!(!cmd.contains("predicates"));
    }

    #[test]
    fn t16_java_1_21_numeric_attribute_id_quoted() {
        let mut f = base(GiveVersion::Java1_21);
        f.attributes = vec![AttributeRow {
            r#type: "armor".to_string(),
            amount: json!(1),
            slot: "any".to_string(),
            operation: "add_value".to_string(),
            id: "123".to_string(),
        }];
        assert!(build(&f).contains(r#"id:"123""#));
    }

    #[test]
    fn t17_java_1_21_11_plus_tooltip_display_hidden_components_quoted() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.hidden_components = "enchantments".to_string();
        assert!(build(&f).contains(r#"hidden_components:["minecraft:enchantments"]"#));
    }

    #[test]
    fn t18_java_1_21_2_text_uses_snbt_single_quoted_strings() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.display_name = vec![vec![json!({"text": "hi"})]];
        f.item_name = vec![vec![json!({"text": "name"})]];
        f.lore = vec![vec![json!({"text": "a"})]];
        let cmd = build(&f);
        assert!(cmd.contains("custom_name='"));
        assert!(cmd.contains("item_name='"));
        assert!(cmd.contains("lore=['"));
    }

    #[test]
    fn t19_java_1_21_2_enchantments_flat_no_levels_wrapper() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
        let cmd = build(&f);
        assert!(cmd.contains("enchantments={unbreaking:3}"));
        assert!(!cmd.contains("levels"));
    }

    #[test]
    fn t20_java_1_21_2_attribute_modern_array_unquoted_type() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.attributes = vec![AttributeRow {
            r#type: "armor".to_string(),
            amount: json!(2),
            slot: "any".to_string(),
            operation: "add_value".to_string(),
            id: "x".to_string(),
        }];
        let cmd = build(&f);
        assert!(cmd.contains("attribute_modifiers=[{type:armor"));
        assert!(!cmd.contains("modifiers:["));
        assert!(cmd.contains(r#"id:"x""#));
    }

    #[test]
    fn t21_java_1_21_2_can_place_on_can_break_predicates_wrapper() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.block_limits = vec![
            BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() },
            BlockLimitRow { block: "minecraft:dirt".to_string(), r#type: "break".to_string() },
        ];
        let cmd = build(&f);
        assert!(cmd.contains(r#"can_place_on={predicates:[{blocks:"minecraft:stone"}]}"#));
        assert!(cmd.contains(r#"can_break={predicates:[{blocks:"minecraft:dirt"}]}"#));
    }

    #[test]
    fn t22_java_1_21_2_supports_glider_death_protection_consumable() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.glider = true;
        f.death_protection = true;
        f.consumable_enabled = true;
        f.consume_seconds = 2.0;
        let cmd = build(&f);
        assert!(cmd.contains("glider={}"));
        assert!(cmd.contains("death_protection"));
        assert!(cmd.contains("consumable={consume_seconds:2"));
    }

    #[test]
    fn t23_java_1_21_2_omits_tooltip_display() {
        let mut f = base(GiveVersion::Java1_21_2);
        f.hidden_components = "enchantments".to_string();
        assert!(!build(&f).contains("tooltip_display"));
    }

    #[test]
    fn t24_java_1_21_3_same_syntax_as_1_21_2() {
        let make = |version: GiveVersion| -> String {
            let mut f = base(version);
            f.display_name = vec![vec![json!({"text": "hi"})]];
            f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
            f.glider = true;
            f.block_limits = vec![BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() }];
            build(&f)
        };
        assert_eq!(make(GiveVersion::Java1_21_3), make(GiveVersion::Java1_21_2));
    }

    #[test]
    fn t25_java_1_21_4_same_syntax_as_1_21_2() {
        let make = |version: GiveVersion| -> String {
            let mut f = base(version);
            f.display_name = vec![vec![json!({"text": "hi"})]];
            f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
            f.glider = true;
            f.block_limits = vec![BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() }];
            build(&f)
        };
        assert_eq!(make(GiveVersion::Java1_21_4), make(GiveVersion::Java1_21_2));
    }

    #[test]
    fn t26_java_1_21_5_same_syntax_as_1_21_11_plus() {
        let make = |version: GiveVersion| -> String {
            let mut f = base(version);
            f.display_name = vec![vec![json!({"text": "hi"})]];
            f.block_limits = vec![BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() }];
            f.hidden_components = "enchantments".to_string();
            build(&f)
        };
        assert_eq!(make(GiveVersion::Java1_21_5), make(GiveVersion::Java1_21_11Plus));
    }

    #[test]
    fn t27_java_1_21_6_same_syntax_as_1_21_11_plus() {
        let mut f1 = base(GiveVersion::Java1_21_6);
        let mut f2 = base(GiveVersion::Java1_21_11Plus);
        f1.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(2) }];
        f2.enchantments = f1.enchantments.clone();
        assert_eq!(build(&f1), build(&f2));
    }

    #[test]
    fn t28_java_1_21_9_same_syntax_as_1_21_11_plus() {
        let mut f1 = base(GiveVersion::Java1_21_9);
        let mut f2 = base(GiveVersion::Java1_21_11Plus);
        f1.death_protection = true;
        f2.death_protection = true;
        assert_eq!(build(&f1), build(&f2));
    }

    #[test]
    fn t29_java_26_1_and_26_2_plus_same_syntax_as_1_21_11_plus() {
        let mut f1 = base(GiveVersion::Java26_1);
        let mut f2 = base(GiveVersion::Java26_2Plus);
        let mut f3 = base(GiveVersion::Java1_21_11Plus);
        f1.glider = true;
        f2.glider = true;
        f3.glider = true;
        f1.hidden_components = "enchantments".to_string();
        f2.hidden_components = "enchantments".to_string();
        f3.hidden_components = "enchantments".to_string();
        assert_eq!(build(&f1), build(&f3));
        assert_eq!(build(&f2), build(&f3));
    }

    #[test]
    fn t30_java_1_20_5_text_uses_snbt_single_quoted_strings() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.display_name = vec![vec![json!({"text": "hi"})]];
        f.item_name = vec![vec![json!({"text": "name"})]];
        f.lore = vec![vec![json!({"text": "a"})]];
        let cmd = build(&f);
        assert!(cmd.contains("custom_name='"));
        assert!(cmd.contains("item_name='"));
        assert!(cmd.contains("lore=['"));
    }

    #[test]
    fn t31_java_1_20_5_can_place_on_predicates_wrapper() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.block_limits = vec![BlockLimitRow { block: "minecraft:stone".to_string(), r#type: "place".to_string() }];
        assert!(build(&f).contains(r#"can_place_on={predicates:[{blocks:"minecraft:stone"}]}"#));
    }

    #[test]
    fn t32_java_1_20_5_no_consumable_glider_death_protection() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.consumable_enabled = true;
        f.consume_seconds = 2.0;
        f.glider = true;
        f.death_protection = true;
        let cmd = build(&f);
        assert!(!cmd.contains("consumable="));
        assert!(!cmd.contains("glider"));
        assert!(!cmd.contains("death_protection"));
    }

    #[test]
    fn t33_java_1_20_5_no_attribute_modifiers() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.attributes = vec![AttributeRow {
            r#type: "armor".to_string(),
            amount: json!(5),
            slot: "任意".to_string(),
            operation: "加算".to_string(),
            id: "test".to_string(),
        }];
        assert!(!build(&f).contains("attribute_modifiers"));
    }

    #[test]
    fn t34_java_1_20_5_no_tooltip_display() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.hidden_components = "enchantments".to_string();
        assert!(!build(&f).contains("tooltip_display"));
    }

    #[test]
    fn t35_java_1_20_5_enchantments_flat_format() {
        let mut f = base(GiveVersion::Java1_20_5);
        f.enchantments = vec![EnchantRow { id: "minecraft:unbreaking".to_string(), level: json!(3) }];
        assert!(build(&f).contains("enchantments={unbreaking:3}"));
    }

    #[test]
    fn t36_font_emitted_modern() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![json!({"text": "A", "font": "minecraft:illageralt"})]];
        assert!(build(&f).contains(r#""font":"minecraft:illageralt""#));
    }

    #[test]
    fn t37_obfuscated_emitted() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![json!({"text": "A", "obfuscated": true})]];
        assert!(build(&f).contains(r#""obfuscated":true"#));
    }

    #[test]
    fn t38_named_color_passthrough() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![json!({"text": "A", "color": "red"})]];
        assert!(build(&f).contains(r#""color":"red""#));
    }

    #[test]
    fn t39_object_sprite_and_player_on_1_21_11_plus() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![
            json!({"type": "object", "object": "atlas", "atlas": "minecraft:blocks", "sprite": "block/stone"}),
            json!({"type": "object", "object": "player", "player": "Notch", "hat": true}),
        ]];
        let cmd = build(&f);
        assert!(cmd.contains(r#"{"type":"object","object":"atlas","atlas":"minecraft:blocks","sprite":"block/stone"}"#));
        assert!(cmd.contains(r#"{"type":"object","object":"player","player":"Notch","hat":true}"#));
    }

    #[test]
    fn t40_object_stripped_on_1_21_6_with_warning() {
        let mut f = base(GiveVersion::Java1_21_6);
        f.display_name = vec![vec![json!({"text": "x"}), json!({"type": "object", "sprite": "item/diamond"})]];
        let (cmd, warnings) = build_w(&f);
        assert!(!cmd.contains(r#""type":"object""#));
        assert!(!warnings.is_empty());
        assert!(cmd.contains(r#"{"text":"x"}"#));
    }

    #[test]
    fn t41_object_stripped_on_1_21_legacy_too() {
        let mut f = base(GiveVersion::Java1_21);
        f.display_name = vec![vec![json!({"type": "object", "sprite": "item/diamond"})]];
        assert!(!build(&f).contains("object"));
    }

    #[test]
    fn t42_click_event_modern_snake_case() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![json!({"text": "c", "click_event": {"action": "run_command", "value": "say hi"}})]];
        assert!(build(&f).contains(r#""click_event":{"action":"run_command","command":"say hi"}"#));
    }

    #[test]
    fn t43_click_event_legacy_camel_case() {
        let mut f = base(GiveVersion::Java1_21);
        f.display_name = vec![vec![json!({"text": "c", "click_event": {"action": "run_command", "value": "/say hi"}})]];
        assert!(build(&f).contains(r#""clickEvent":{"action":"run_command","value":"/say hi"}"#));
    }

    #[test]
    fn t44_hover_event_modern_show_text() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.display_name = vec![vec![json!({"text": "h", "hover_event": {"action": "show_text", "text": [{"text": "tip"}]}})]];
        assert!(build(&f).contains(r#""hover_event":{"action":"show_text","value":[{"text":"tip"}]}"#));
    }

    #[test]
    fn t45_hover_event_legacy_contents() {
        let mut f = base(GiveVersion::Java1_21);
        f.display_name = vec![vec![json!({"text": "h", "hover_event": {"action": "show_text", "text": [{"text": "tip"}]}})]];
        assert!(build(&f).contains(r#""hoverEvent":{"action":"show_text","contents":[{"text":"tip"}]}"#));
    }

    #[test]
    fn t46_translatable_with_args() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.lore = vec![vec![json!({"type": "translatable", "translate": "item.minecraft.stone", "with": [{"text": "arg"}]})]];
        let cmd = build(&f);
        assert!(cmd.contains(r#""translate":"item.minecraft.stone""#));
        assert!(cmd.contains(r#""with":[{"text":"arg"}]"#));
    }

    #[test]
    fn t47_keybind_selector_score_nbt() {
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.lore = vec![
            vec![json!({"type": "keybind", "keybind": "key.jump"})],
            vec![json!({"type": "selector", "selector": "@p"})],
            vec![json!({"type": "score", "score": {"name": "@s", "objective": "kills"}})],
            vec![json!({"type": "nbt", "nbt": "Health", "source": "entity", "entity": "@s", "interpret": true})],
        ];
        let cmd = build(&f);
        assert!(cmd.contains(r#"{"keybind":"key.jump"}"#));
        assert!(cmd.contains(r#"{"selector":"@p"}"#));
        assert!(cmd.contains(r#"{"score":{"name":"@s","objective":"kills"}}"#));
        assert!(cmd.contains(r#""nbt":"Health","source":"entity","entity":"@s","interpret":true"#));
    }

    #[test]
    fn t48_shadow_color_array_kept_on_1_21_4_plus_converted_below() {
        let mut f = base(GiveVersion::Java1_21_4);
        f.display_name = vec![vec![json!({"text": "s", "shadow_color": [1, 0, 0, 1]})]];
        assert!(build(&f).contains(r#""shadow_color":[1,0,0,1]"#));

        let mut f2 = base(GiveVersion::Java1_21_2);
        f2.display_name = vec![vec![json!({"text": "s", "shadow_color": [1, 0, 0, 1]})]];
        assert!(build(&f2).contains(r#""shadow_color":-65536"#));
    }

    #[test]
    fn custom_data_component() {
        // 用于 AI 模式给抛射物打标记，见 K8 探针实测。
        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.custom_data = "{soul_tnt_arrow:1b}".to_string();
        assert_eq!(build(&f), "give @a minecraft:stone[custom_data={soul_tnt_arrow:1b}] 1");

        let mut f = base(GiveVersion::Java1_21); // 走 buildJava121Legacy 分支
        f.custom_data = "{soul_tnt_arrow:1b}".to_string();
        assert_eq!(build(&f), "give @a minecraft:stone[custom_data={soul_tnt_arrow:1b}] 1");

        let mut f = base(GiveVersion::Java1_21_11Plus);
        f.custom_data = "   ".to_string(); // 空白应视为未设置
        assert_eq!(build(&f), "give @a minecraft:stone 1");
    }

    #[test]
    fn detect_give_version_from_raw_matches_ts() {
        assert_eq!(detect_give_version_from_raw("1.20.5"), Some(GiveVersion::Java1_20_5));
        assert_eq!(detect_give_version_from_raw("1.20.6"), Some(GiveVersion::Java1_20_5));
        assert_eq!(detect_give_version_from_raw("1.20.4"), None);
        assert_eq!(detect_give_version_from_raw("1.21"), Some(GiveVersion::Java1_21));
        assert_eq!(detect_give_version_from_raw("1.21.1"), Some(GiveVersion::Java1_21_1));
        assert_eq!(detect_give_version_from_raw("1.21.2"), Some(GiveVersion::Java1_21_2));
        assert_eq!(detect_give_version_from_raw("1.21.3"), Some(GiveVersion::Java1_21_3));
        assert_eq!(detect_give_version_from_raw("1.21.4"), Some(GiveVersion::Java1_21_4));
        assert_eq!(detect_give_version_from_raw("1.21.5"), Some(GiveVersion::Java1_21_5));
        assert_eq!(detect_give_version_from_raw("1.21.6"), Some(GiveVersion::Java1_21_6));
        assert_eq!(detect_give_version_from_raw("1.21.7"), Some(GiveVersion::Java1_21_6));
        assert_eq!(detect_give_version_from_raw("1.21.8"), Some(GiveVersion::Java1_21_6));
        assert_eq!(detect_give_version_from_raw("1.21.9"), Some(GiveVersion::Java1_21_9));
        assert_eq!(detect_give_version_from_raw("1.21.10"), Some(GiveVersion::Java1_21_9));
        assert_eq!(detect_give_version_from_raw("1.21.11"), Some(GiveVersion::Java1_21_11Plus));
        assert_eq!(detect_give_version_from_raw("1.21.20"), Some(GiveVersion::Java1_21_11Plus));
        assert_eq!(detect_give_version_from_raw("26.1"), Some(GiveVersion::Java26_1));
        assert_eq!(detect_give_version_from_raw("26.2"), Some(GiveVersion::Java26_2Plus));
        assert_eq!(detect_give_version_from_raw("26.3"), Some(GiveVersion::Java26_2Plus));
        assert_eq!(detect_give_version_from_raw("27.0"), Some(GiveVersion::Java26_2Plus));
        assert_eq!(detect_give_version_from_raw(""), None);
        assert_eq!(detect_give_version_from_raw("not-a-version"), None);
        assert_eq!(detect_give_version_from_raw("1.19"), None);
    }

    // ---------------- 未被 108 条金标准覆盖、但属于本次移植范围的补充测试 ----------------

    #[test]
    fn color_helpers_roundtrip() {
        assert_eq!(hex_to_rgb("#000599").unwrap(), (0x00, 0x05, 0x99));
        assert!(hex_to_rgb("not-a-color").is_err());
        assert_eq!(rgb_to_hex((0, 5, 0x99)), "#000599");
        let grad = color_lerp("#000000", "#ffffff", 3).unwrap();
        assert_eq!(grad, vec!["#000000".to_string(), "#808080".to_string(), "#ffffff".to_string()]);
        assert_eq!(shadow_color_int("#ff0000", 100.0).unwrap(), -65536);
    }

    #[test]
    fn normalize_form_defensive_against_missing_and_dirty_fields() {
        let form = normalize_form(&json!({
            "version": "not_a_real_version",
            "count": "not-a-number",
            "enchantments": "not-an-array",
        }));
        assert_eq!(form.version, GiveVersion::Java1_21_11Plus);
        assert_eq!(form.count, 1);
        assert!(form.enchantments.is_empty());
        assert_eq!(form.item, "石头");
    }

    #[test]
    fn normalize_form_on_non_object_returns_default() {
        let form = normalize_form(&json!("just a string"));
        assert_eq!(form.item, create_default_form().item);
    }

    #[test]
    fn get_modern_profile_matches_family_flags() {
        assert!(!get_modern_profile(GiveVersion::Bedrock).supports_glider);
        assert!(!get_modern_profile(GiveVersion::Java1_21).supports_attribute_modifiers);
        assert!(!get_modern_profile(GiveVersion::Java1_20_5).supports_consumable);
        assert!(get_modern_profile(GiveVersion::Java1_21_2).supports_glider);
        assert!(get_modern_profile(GiveVersion::Java1_21_11Plus).supports_tooltip_display);
    }

    #[test]
    fn is_modern_nbt_family_boundary() {
        assert!(!is_modern_nbt_family(GiveVersion::Java1_21_4));
        assert!(is_modern_nbt_family(GiveVersion::Java1_21_5));
        assert!(is_modern_nbt_family(GiveVersion::Java26_2Plus));
        assert!(!is_modern_nbt_family(GiveVersion::Bedrock));
    }
}
