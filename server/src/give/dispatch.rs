//! 指令分派器（AI 意图 → 确定性命令字符串）。移植自客户端 `src/logic/dispatch.ts`。
//!
//! AI 只负责把自然语言翻译成「指令意图」（[`CommandIntent`]）——描述"做什么"，
//! 不负责拼写 1.20.5+ 的精确组件/NBT 语法（AI 在这上面极易出错）。
//! 真正的语法生成交给 `commands/*` 下经 mc-verifier 实证过的确定性构建器。
//!
//! 这样 AI 幻觉只会影响"意图"，不会产出语法非法的命令——非法意图在此被捕获并报错。
//!
//! `form` 字段用 `serde_json::Value` 承载：AI 产出的意图数据经常缺字段/类型不严格，
//! 用宽松的 JSON Value 比强类型 struct 更贴近 TS 原版"宽容"的设计意图——各 command
//! 分支自己负责把 Value 转换成对应构建器要的强类型表单，缺字段就地取默认值，
//! 和 `builder.rs::normalize_form` 对脏数据的处理思路一致。

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::give::builder::{
    build_give_command, normalize_form, GiveVersion, RichLine,
};
use crate::give::catalog::{catalog_miss, particle_id_only, GiveCatalogs, Indexed};
use crate::give::commands::attribute::{
    build_attribute_command, AttributeAction, AttributeForm, AttributeOperation,
};
use crate::give::commands::clone::{
    build_clone_command, CloneForm, CloneMaskMode, CloneMode,
};
use crate::give::commands::effect::{
    build_effect_clear_command, build_effect_give_command, EffectClearForm, EffectDuration,
    EffectGiveForm,
};
use crate::give::commands::enchant::{build_enchant_command, EnchantForm};
use crate::give::commands::execute::{build_execute_command, ExecuteForm};
use crate::give::commands::fill::{build_fill_command, BlockFilter, FillForm, FillMode};
use crate::give::commands::nbt::{NbtAttribute, NbtEffect, NbtEnchantment, NbtEquipment, NbtItem};
use crate::give::commands::particle::{build_particle_command, ParticleForm, ParticleMode};
use crate::give::commands::say::{build_say_command, SayForm};
use crate::give::commands::scoreboard::{
    build_scoreboard_command, ScoreboardAction, ScoreboardForm, ScoreboardOperation,
};
use crate::give::commands::setblock::{
    build_setblock_command, ContainerSlot, SetblockCommandBlockOptions, SetblockForm, SetblockMode,
    SetblockNbt,
};
use crate::give::commands::summon::{build_summon_command, SummonForm, SummonPassenger};
use crate::give::commands::tp::{build_tp_command, TpCoordsForm, TpEntityForm, TpForm};

// =====================================================================================
// CommandIntent
// =====================================================================================

/// AI 产出的单条指令意图。`form` 为对应构建器的（可能不完整的）表单数据。
///
/// 对应客户端 `src/logic/dispatch.ts::CommandIntent`。手写 `Deserialize`/`Serialize`
/// 而不是单纯 `#[serde(tag = "command", content = "form")]` 派生，是为了让"AI 编造了
/// 一个不存在的 command"这种情况能被正常反序列化成 `Unknown` 变体走到 dispatch 里
/// 报错，而不是在反序列化阶段就直接失败——这与 TS 版本里 `command` 是宽松字符串、
/// 由 `dispatchIntent` 的 `default` 分支兜底报错的行为一致。
#[derive(Debug, Clone)]
pub enum CommandIntent {
    Give(Value),
    Say(Value),
    EffectGive(Value),
    EffectClear(Value),
    Tp(Value),
    Setblock(Value),
    Summon(Value),
    Fill(Value),
    Clone(Value),
    Enchant(Value),
    Execute(Value),
    Scoreboard(Value),
    Attribute(Value),
    Particle(Value),
    /// 未知的 command 类型；保留原始字符串以便报错文案里回显。
    Unknown(String, Value),
}

impl CommandIntent {
    /// 对应 TS 里 `intent.command`。
    pub fn command_name(&self) -> &str {
        match self {
            CommandIntent::Give(_) => "give",
            CommandIntent::Say(_) => "say",
            CommandIntent::EffectGive(_) => "effect_give",
            CommandIntent::EffectClear(_) => "effect_clear",
            CommandIntent::Tp(_) => "tp",
            CommandIntent::Setblock(_) => "setblock",
            CommandIntent::Summon(_) => "summon",
            CommandIntent::Fill(_) => "fill",
            CommandIntent::Clone(_) => "clone",
            CommandIntent::Enchant(_) => "enchant",
            CommandIntent::Execute(_) => "execute",
            CommandIntent::Scoreboard(_) => "scoreboard",
            CommandIntent::Attribute(_) => "attribute",
            CommandIntent::Particle(_) => "particle",
            CommandIntent::Unknown(name, _) => name.as_str(),
        }
    }

    /// 对应 TS 里 `intent.form`。
    pub fn form(&self) -> &Value {
        match self {
            CommandIntent::Give(f)
            | CommandIntent::Say(f)
            | CommandIntent::EffectGive(f)
            | CommandIntent::EffectClear(f)
            | CommandIntent::Tp(f)
            | CommandIntent::Setblock(f)
            | CommandIntent::Summon(f)
            | CommandIntent::Fill(f)
            | CommandIntent::Clone(f)
            | CommandIntent::Enchant(f)
            | CommandIntent::Execute(f)
            | CommandIntent::Scoreboard(f)
            | CommandIntent::Attribute(f)
            | CommandIntent::Particle(f)
            | CommandIntent::Unknown(_, f) => f,
        }
    }

    fn new(command: &str, form: Value) -> Self {
        match command {
            "give" => CommandIntent::Give(form),
            "say" => CommandIntent::Say(form),
            "effect_give" => CommandIntent::EffectGive(form),
            "effect_clear" => CommandIntent::EffectClear(form),
            "tp" => CommandIntent::Tp(form),
            "setblock" => CommandIntent::Setblock(form),
            "summon" => CommandIntent::Summon(form),
            "fill" => CommandIntent::Fill(form),
            "clone" => CommandIntent::Clone(form),
            "enchant" => CommandIntent::Enchant(form),
            "execute" => CommandIntent::Execute(form),
            "scoreboard" => CommandIntent::Scoreboard(form),
            "attribute" => CommandIntent::Attribute(form),
            "particle" => CommandIntent::Particle(form),
            other => CommandIntent::Unknown(other.to_string(), form),
        }
    }
}

#[derive(Deserialize)]
struct RawIntent {
    command: String,
    #[serde(default)]
    form: Value,
}

impl<'de> Deserialize<'de> for CommandIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawIntent::deserialize(deserializer)?;
        Ok(CommandIntent::new(&raw.command, raw.form))
    }
}

impl Serialize for CommandIntent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Out<'a> {
            command: &'a str,
            form: &'a Value,
        }
        Out { command: self.command_name(), form: self.form() }.serialize(serializer)
    }
}

/// 分派结果。对应客户端 `DispatchResult`。
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// 原始意图（便于 UI 回显 / 调试）。
    pub intent: CommandIntent,
    /// 生成的命令字符串；失败时为 None。
    pub command: Option<String>,
    /// 失败原因；成功时为 None。
    pub error: Option<String>,
    /// 是否需要每 tick 持续执行（目前只有 execute 意图能标记 form.loop=true）。
    /// UI 据此区分"一次性指令，可直接复制"和"循环侦测，需要部署成 datapack"。
    /// 字段名沿用 `commands::execute::ExecuteForm` 里 `r#loop` 的命名约定。
    pub r#loop: bool,
}

// =====================================================================================
// Value 取值小工具
// =====================================================================================

fn as_obj(v: &Value) -> Option<&serde_json::Map<String, Value>> {
    v.as_object()
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    as_obj(v).and_then(|o| o.get(key))
}

fn v_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    get(v, key).and_then(Value::as_str)
}

fn v_str_owned(v: &Value, key: &str) -> Option<String> {
    v_str(v, key).map(str::to_string)
}

fn v_str_or(v: &Value, key: &str, fallback: &str) -> String {
    v_str(v, key).unwrap_or(fallback).to_string()
}

fn v_bool(v: &Value, key: &str) -> bool {
    match get(v, key) {
        Some(Value::Bool(b)) => *b,
        Some(other) => truthy(other),
        None => false,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn v_i64(v: &Value, key: &str) -> Option<i64> {
    get(v, key).and_then(value_to_i64)
}

fn value_to_i64(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64))
}

fn v_f64(v: &Value, key: &str) -> Option<f64> {
    get(v, key).and_then(Value::as_f64)
}

fn v_arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    match get(v, key).and_then(Value::as_array) {
        Some(a) => a.as_slice(),
        None => &[],
    }
}

fn v_bool_opt(v: &Value, key: &str) -> Option<bool> {
    get(v, key).map(truthy)
}

/// `[string, string, string]` 坐标数组；缺失/非法元素用空字符串占位。
fn v_coords3(v: &Value, key: &str) -> [String; 3] {
    let arr = v_arr(v, key);
    let at = |i: usize| arr.get(i).and_then(Value::as_str).unwrap_or("").to_string();
    [at(0), at(1), at(2)]
}

// =====================================================================================
// 目录存在性校验（AI 幻觉防线之二）
// =====================================================================================
//
// mapCatalog 对手动模式很宽容——匹配不上就假设是模组物品/自定义 id，直接
// namespaced() 放行，这是刻意的（手动模式的用户可能真的要填模组内容）。
// 但 AI 生成的内容不该有这种自由度：系统提示词已经把官方目录喂给它了，
// 匹配不上目录，几乎总是编造，必须在这里拦下来，而不是让它悄悄拼进最终命令。
// 只有 dispatchIntent/dispatchIntents 会走这层校验，手动模式直接调 builder，
// 不受影响。

/// 校验一组 `{ id }` 形状的附魔/效果条目（give.enchantments、summon.effects 等）。
fn first_catalog_miss_in_list(kind: &str, list: Option<&Value>, cat: &Indexed) -> Option<String> {
    let arr = list?.as_array()?;
    for row in arr {
        let raw = row.get("id").and_then(Value::as_str);
        if let Some(err) = catalog_miss(kind, raw, cat) {
            return Some(err);
        }
    }
    None
}

/// 校验 summon.equipment：`{ mainhand?: { id, enchantments? }, head?: ..., ... }`。
fn equipment_catalog_miss(equipment: Option<&Value>, version: GiveVersion) -> Option<String> {
    let obj = equipment?.as_object()?;
    for slot in obj.values() {
        let slot_obj = slot.as_object();
        if slot_obj.is_none() {
            continue;
        }
        let id = slot.get("id").and_then(Value::as_str);
        let err = catalog_miss("物品", id, GiveCatalogs::items(version))
            .or_else(|| first_catalog_miss_in_list("附魔", slot.get("enchantments"), GiveCatalogs::enchants()));
        if err.is_some() {
            return err;
        }
    }
    None
}

/// 校验 summon.passengers[]：每个乘客也是一个实体，同样不能是编出来的。
fn passengers_catalog_miss(passengers: Option<&Value>, version: GiveVersion) -> Option<String> {
    let arr = passengers?.as_array()?;
    for p in arr {
        if !p.is_object() {
            continue;
        }
        let entity_type = p.get("entityType").and_then(Value::as_str);
        if let Some(err) = catalog_miss("乘客实体类型", entity_type, GiveCatalogs::entities(version)) {
            return Some(err);
        }
    }
    None
}

/// 校验 setblock.containerItems[]：`{ slot, item: { id, enchantments? } }`。
fn container_items_catalog_miss(items: Option<&Value>, version: GiveVersion) -> Option<String> {
    let arr = items?.as_array()?;
    for entry in arr {
        let item = match entry.get("item") {
            Some(i) if i.is_object() => i,
            _ => continue,
        };
        let id = item.get("id").and_then(Value::as_str);
        let err = catalog_miss("容器内物品", id, GiveCatalogs::items(version))
            .or_else(|| first_catalog_miss_in_list("附魔", item.get("enchantments"), GiveCatalogs::enchants()));
        if err.is_some() {
            return err;
        }
    }
    None
}

/// 校验 fill.replaceFilter / clone.filter 这种 `{ block, blockstate? }` 过滤器。
fn block_filter_catalog_miss(kind: &str, filter: Option<&Value>, version: GiveVersion) -> Option<String> {
    let obj = filter?;
    if !obj.is_object() {
        return None;
    }
    let block = obj.get("block").and_then(Value::as_str);
    catalog_miss(kind, block, GiveCatalogs::blocks(version))
}

/// 校验 scoreboard 判据里内嵌的物品 id。
///
/// 提示词主动教了 minecraft.used:minecraft.<item> / minecraft.custom:minecraft.<stat>
/// 这类统计判据，冒号后面那截是真实的物品 id，编错了整个计分板就永远不会涨分，
/// 而且失败得很安静——不报错，只是没反应。只挑 used/mined/crafted/broken/
/// picked_up/dropped 这几类"后面接物品 id"的判据来查；custom 后面接的是统计项名
/// （sneak_time、jump 之类），不在物品表里，跳过。
const ITEM_BACKED_CRITERIA_PREFIXES: &[&str] = &[
    "minecraft.used",
    "minecraft.mined",
    "minecraft.crafted",
    "minecraft.broken",
    "minecraft.picked_up",
    "minecraft.dropped",
];

fn criteria_catalog_miss(criteria: Option<&Value>, version: GiveVersion) -> Option<String> {
    let criteria = criteria?.as_str()?;
    let colon = criteria.find(':')?;
    let (head, tail) = (&criteria[..colon], &criteria[colon + 1..]);
    if !ITEM_BACKED_CRITERIA_PREFIXES.iter().any(|p| head == *p) {
        return None;
    }
    // 判据里用点号分隔命名空间（minecraft.stone），转成正常 id 再查；
    // 对应 TS `tail.replace(".", ":")`——只替换第一个点号。
    let as_id = tail.replacen('.', ":", 1);
    catalog_miss("计分板判据里的物品", Some(as_id.as_str()), GiveCatalogs::items(version))
}

/// 按意图类型校验涉及官方目录的字段。返回 `Some` 即视为构建失败。
/// 对应 `dispatch.ts::validateIntentCatalog`。
fn validate_intent_catalog(intent: &CommandIntent, version: GiveVersion) -> Option<String> {
    let form = intent.form();
    match intent {
        CommandIntent::Give(_) => catalog_miss("物品", v_str(form, "item"), GiveCatalogs::items(version))
            .or_else(|| first_catalog_miss_in_list("附魔", get(form, "enchantments"), GiveCatalogs::enchants())),
        CommandIntent::Setblock(_) => catalog_miss("方块", v_str(form, "block"), GiveCatalogs::blocks(version))
            .or_else(|| container_items_catalog_miss(get(form, "containerItems"), version)),
        CommandIntent::Fill(_) => catalog_miss("方块", v_str(form, "block"), GiveCatalogs::blocks(version))
            .or_else(|| block_filter_catalog_miss("替换过滤方块", get(form, "replaceFilter"), version)),
        CommandIntent::Clone(_) => block_filter_catalog_miss("克隆过滤方块", get(form, "filter"), version),
        CommandIntent::Enchant(_) => catalog_miss("附魔", v_str(form, "enchantment"), GiveCatalogs::enchants()),
        CommandIntent::EffectGive(_) => catalog_miss("药水效果", v_str(form, "effect"), GiveCatalogs::effects()),
        // effect_give 一直有校验，effect_clear 之前漏了，两者不该不一致
        CommandIntent::EffectClear(_) => catalog_miss("药水效果", v_str(form, "effect"), GiveCatalogs::effects()),
        CommandIntent::Attribute(_) => {
            catalog_miss("属性", v_str(form, "attribute"), GiveCatalogs::attributes())
        }
        CommandIntent::Scoreboard(_) => {
            let action = get(form, "action");
            let criteria = action.and_then(|a| a.get("criteria"));
            criteria_catalog_miss(criteria, version)
        }
        CommandIntent::Summon(_) => {
            catalog_miss("实体类型", v_str(form, "entityType"), GiveCatalogs::entities(version))
                .or_else(|| first_catalog_miss_in_list("药水效果", get(form, "effects"), GiveCatalogs::effects()))
                .or_else(|| equipment_catalog_miss(get(form, "equipment"), version))
                .or_else(|| passengers_catalog_miss(get(form, "passengers"), version))
        }
        CommandIntent::Particle(_) => {
            let name = v_str(form, "name");
            name.and_then(|n| catalog_miss("粒子", Some(particle_id_only(n)), GiveCatalogs::particles()))
        }
        CommandIntent::Say(_)
        | CommandIntent::Tp(_)
        | CommandIntent::Execute(_)
        | CommandIntent::Unknown(_, _) => None,
    }
}

// =====================================================================================
// Value -> Form 转换（各 command 分支）
// =====================================================================================

fn parse_enchant_row_nbt(v: &Value) -> Option<NbtEnchantment> {
    let id = v.get("id").and_then(Value::as_str)?.to_string();
    let level = v.get("level").and_then(value_to_i64).unwrap_or(1);
    Some(NbtEnchantment { id, level })
}

fn parse_enchantments_field(v: &Value, key: &str) -> Option<Vec<NbtEnchantment>> {
    let arr = get(v, key)?.as_array()?;
    let rows: Vec<NbtEnchantment> = arr.iter().filter_map(parse_enchant_row_nbt).collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

fn parse_nbt_item(v: &Value) -> NbtItem {
    NbtItem {
        id: v_str_or(v, "id", ""),
        count: v_i64(v, "count"),
        enchantments: parse_enchantments_field(v, "enchantments"),
        components: Vec::new(),
    }
}

fn parse_rich_line(v: &Value) -> RichLine {
    v.as_array().cloned().unwrap_or_default()
}

fn giveversion_json(version: GiveVersion) -> Value {
    serde_json::to_value(version).expect("GiveVersion 序列化不会失败")
}

fn build_give(form: &Value, version: GiveVersion) -> Result<String, String> {
    let mut with_version = form.clone();
    if let Some(obj) = with_version.as_object_mut() {
        obj.insert("version".to_string(), giveversion_json(version));
    } else {
        with_version = serde_json::json!({ "version": giveversion_json(version) });
    }
    let gform = normalize_form(&with_version);
    let mut warnings = Vec::new();
    Ok(build_give_command(&gform, &mut warnings))
}

fn build_say(form: &Value) -> Result<String, String> {
    let f = SayForm { with_slash: v_bool(form, "withSlash"), message: v_str_or(form, "message", "") };
    Ok(build_say_command(&f))
}

fn build_enchant(form: &Value) -> Result<String, String> {
    let f = EnchantForm {
        with_slash: v_bool(form, "withSlash"),
        targets: v_str_or(form, "targets", ""),
        enchantment: v_str_or(form, "enchantment", ""),
        level: v_i64(form, "level"),
    };
    Ok(build_enchant_command(&f))
}

fn parse_effect_duration(v: &Value, key: &str) -> Option<EffectDuration> {
    match get(v, key) {
        Some(Value::String(s)) if s == "infinite" => Some(EffectDuration::Infinite),
        Some(other) => value_to_i64(other).map(EffectDuration::Seconds),
        None => None,
    }
}

fn build_effect_give(form: &Value) -> Result<String, String> {
    let f = EffectGiveForm {
        with_slash: v_bool(form, "withSlash"),
        target: v_str_or(form, "target", ""),
        effect: v_str_or(form, "effect", ""),
        duration: parse_effect_duration(form, "duration"),
        amplifier: v_i64(form, "amplifier"),
        hide_particles: v_bool_opt(form, "hideParticles"),
    };
    Ok(build_effect_give_command(&f))
}

fn build_effect_clear(form: &Value) -> Result<String, String> {
    let f = EffectClearForm {
        with_slash: v_bool(form, "withSlash"),
        target: v_str_or(form, "target", ""),
        effect: v_str_owned(form, "effect"),
    };
    Ok(build_effect_clear_command(&f))
}

fn build_tp(form: &Value) -> Result<String, String> {
    let with_slash = v_bool(form, "withSlash");
    let use_teleport_alias = v_bool(form, "useTeleportAlias");
    let targets = v_str_or(form, "targets", "");
    let tf = if get(form, "destination").is_some() {
        TpForm::Entity(TpEntityForm {
            with_slash,
            use_teleport_alias,
            targets,
            destination: v_str_or(form, "destination", ""),
        })
    } else {
        TpForm::Coords(TpCoordsForm {
            with_slash,
            use_teleport_alias,
            targets,
            x: v_str_or(form, "x", ""),
            y: v_str_or(form, "y", ""),
            z: v_str_or(form, "z", ""),
            y_rot: v_str_owned(form, "yRot"),
            x_rot: v_str_owned(form, "xRot"),
            facing_x: v_str_owned(form, "facingX"),
            facing_y: v_str_owned(form, "facingY"),
            facing_z: v_str_owned(form, "facingZ"),
        })
    };
    Ok(build_tp_command(&tf))
}

fn build_fill(form: &Value) -> Result<String, String> {
    let mode = v_str(form, "mode").and_then(|s| match s {
        "replace" => Some(FillMode::Replace),
        "destroy" => Some(FillMode::Destroy),
        "hollow" => Some(FillMode::Hollow),
        "keep" => Some(FillMode::Keep),
        "outline" => Some(FillMode::Outline),
        _ => None,
    });
    let replace_filter = get(form, "replaceFilter").and_then(|v| {
        if !v.is_object() {
            return None;
        }
        Some(BlockFilter { block: v_str_or(v, "block", ""), blockstate: v_str_owned(v, "blockstate") })
    });
    let f = FillForm {
        with_slash: v_bool(form, "withSlash"),
        from: v_coords3(form, "from"),
        to: v_coords3(form, "to"),
        block: v_str_or(form, "block", ""),
        blockstate: v_str_owned(form, "blockstate"),
        nbt: v_str_owned(form, "nbt"),
        mode,
        replace_filter,
    };
    Ok(build_fill_command(&f))
}

fn build_clone(form: &Value) -> Result<String, String> {
    let mask_mode = v_str(form, "maskMode").and_then(|s| match s {
        "replace" => Some(CloneMaskMode::Replace),
        "masked" => Some(CloneMaskMode::Masked),
        "filtered" => Some(CloneMaskMode::Filtered),
        _ => None,
    });
    let clone_mode = v_str(form, "cloneMode").and_then(|s| match s {
        "normal" => Some(CloneMode::Normal),
        "force" => Some(CloneMode::Force),
        "move" => Some(CloneMode::Move),
        _ => None,
    });
    let filter = get(form, "filter").and_then(|v| {
        if !v.is_object() {
            return None;
        }
        Some(BlockFilter { block: v_str_or(v, "block", ""), blockstate: v_str_owned(v, "blockstate") })
    });
    let f = CloneForm {
        with_slash: v_bool(form, "withSlash"),
        from_dimension: v_str_owned(form, "fromDimension"),
        begin: v_coords3(form, "begin"),
        end: v_coords3(form, "end"),
        to_dimension: v_str_owned(form, "toDimension"),
        destination: v_coords3(form, "destination"),
        mask_mode,
        filter,
        clone_mode,
    };
    build_clone_command(&f)
}

fn build_execute(form: &Value) -> Result<String, String> {
    let subcommands: Vec<String> = v_arr(form, "subcommands")
        .iter()
        .map(|v| v.as_str().map(str::to_string).unwrap_or_default())
        .collect();
    let f = ExecuteForm {
        with_slash: v_bool(form, "withSlash"),
        subcommands,
        run: v_str_owned(form, "run"),
        r#loop: v_bool(form, "loop"),
    };
    build_execute_command(&f)
}

fn parse_scoreboard_action(v: &Value) -> Result<ScoreboardAction, String> {
    let kind = v.get("kind").and_then(Value::as_str).unwrap_or("");
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let opt_s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let i = |k: &str| v.get(k).and_then(value_to_i64).unwrap_or(0);
    match kind {
        "objectives_add" => Ok(ScoreboardAction::ObjectivesAdd {
            objective: s("objective"),
            criteria: s("criteria"),
            display_name: opt_s("displayName"),
        }),
        "objectives_remove" => Ok(ScoreboardAction::ObjectivesRemove { objective: s("objective") }),
        "objectives_list" => Ok(ScoreboardAction::ObjectivesList),
        "objectives_setdisplay" => {
            Ok(ScoreboardAction::ObjectivesSetdisplay { slot: s("slot"), objective: opt_s("objective") })
        }
        "objectives_modify_displayname" => Ok(ScoreboardAction::ObjectivesModifyDisplayname {
            objective: s("objective"),
            display_name: s("displayName"),
        }),
        "objectives_modify_rendertype" => {
            let rendertype = match v.get("rendertype").and_then(Value::as_str) {
                Some("hearts") => crate::give::commands::scoreboard::RenderType::Hearts,
                _ => crate::give::commands::scoreboard::RenderType::Integer,
            };
            Ok(ScoreboardAction::ObjectivesModifyRendertype { objective: s("objective"), rendertype })
        }
        "players_set" => Ok(ScoreboardAction::PlayersSet { targets: s("targets"), objective: s("objective"), score: i("score") }),
        "players_add" => Ok(ScoreboardAction::PlayersAdd { targets: s("targets"), objective: s("objective"), score: i("score") }),
        "players_remove" => {
            Ok(ScoreboardAction::PlayersRemove { targets: s("targets"), objective: s("objective"), score: i("score") })
        }
        "players_get" => Ok(ScoreboardAction::PlayersGet { target: s("target"), objective: s("objective") }),
        "players_reset" => Ok(ScoreboardAction::PlayersReset { targets: s("targets"), objective: opt_s("objective") }),
        "players_enable" => Ok(ScoreboardAction::PlayersEnable { targets: s("targets"), objective: s("objective") }),
        "players_list" => Ok(ScoreboardAction::PlayersList { target: opt_s("target") }),
        "players_operation" => {
            let operation = match v.get("operation").and_then(Value::as_str) {
                Some("=") => ScoreboardOperation::Set,
                Some("+=") => ScoreboardOperation::Add,
                Some("-=") => ScoreboardOperation::Sub,
                Some("*=") => ScoreboardOperation::Mul,
                Some("/=") => ScoreboardOperation::Div,
                Some("%=") => ScoreboardOperation::Mod,
                Some("<") => ScoreboardOperation::Lt,
                Some(">") => ScoreboardOperation::Gt,
                Some("><") => ScoreboardOperation::Swap,
                _ => ScoreboardOperation::Set,
            };
            Ok(ScoreboardAction::PlayersOperation {
                targets: s("targets"),
                objective: s("objective"),
                operation,
                source: s("source"),
                source_objective: s("sourceObjective"),
            })
        }
        other => Err(format!("未知 scoreboard 动作: {}", serde_json::to_string(&serde_json::json!({"kind": other})).unwrap_or_default())),
    }
}

fn build_scoreboard(form: &Value) -> Result<String, String> {
    let action_value = get(form, "action").cloned().unwrap_or(Value::Null);
    let action = parse_scoreboard_action(&action_value)?;
    let f = ScoreboardForm { with_slash: v_bool(form, "withSlash"), action };
    Ok(build_scoreboard_command(&f))
}

fn parse_attribute_action(v: &Value) -> Result<AttributeAction, String> {
    let kind = v.get("kind").and_then(Value::as_str).unwrap_or("");
    let scale = v.get("scale").and_then(Value::as_f64);
    let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let opt_s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    match kind {
        "get" => Ok(AttributeAction::Get { scale }),
        "base_get" => Ok(AttributeAction::BaseGet { scale }),
        "base_set" => Ok(AttributeAction::BaseSet { value: f("value") }),
        "modifier_add" => {
            let operation = match v.get("operation").and_then(Value::as_str) {
                Some("multiply_base") => AttributeOperation::MultiplyBase,
                Some("multiply_total") => AttributeOperation::MultiplyTotal,
                _ => AttributeOperation::Add,
            };
            Ok(AttributeAction::ModifierAdd { id: s("id"), name: opt_s("name"), value: f("value"), operation })
        }
        "modifier_remove" => Ok(AttributeAction::ModifierRemove { id: s("id") }),
        "modifier_value_get" => Ok(AttributeAction::ModifierValueGet { id: s("id"), scale }),
        other => Err(format!("未知 attribute 动作: {}", serde_json::to_string(&serde_json::json!({"kind": other})).unwrap_or_default())),
    }
}

fn build_attribute(form: &Value, version: GiveVersion) -> Result<String, String> {
    let action_value = get(form, "action").cloned().unwrap_or(Value::Null);
    let action = parse_attribute_action(&action_value)?;
    let f = AttributeForm {
        version,
        with_slash: v_bool(form, "withSlash"),
        target: v_str_or(form, "target", ""),
        attribute: v_str_or(form, "attribute", ""),
        action,
    };
    Ok(build_attribute_command(&f))
}

fn build_particle(form: &Value) -> Result<String, String> {
    let mode = v_str(form, "mode").and_then(|s| match s {
        "force" => Some(ParticleMode::Force),
        "normal" => Some(ParticleMode::Normal),
        _ => None,
    });
    let f = ParticleForm {
        with_slash: v_bool(form, "withSlash"),
        name: v_str_or(form, "name", ""),
        x: v_str_owned(form, "x"),
        y: v_str_owned(form, "y"),
        z: v_str_owned(form, "z"),
        dx: v_f64(form, "dx"),
        dy: v_f64(form, "dy"),
        dz: v_f64(form, "dz"),
        speed: v_f64(form, "speed"),
        count: v_i64(form, "count"),
        mode,
        viewers: v_str_owned(form, "viewers"),
    };
    Ok(build_particle_command(&f))
}

fn parse_setblock_mode(v: &Value) -> Option<SetblockMode> {
    v_str(v, "mode").and_then(|s| match s {
        "replace" => Some(SetblockMode::Replace),
        "destroy" => Some(SetblockMode::Destroy),
        "keep" => Some(SetblockMode::Keep),
        _ => None,
    })
}

fn parse_container_items(v: &Value) -> Option<Vec<ContainerSlot>> {
    let arr = get(v, "containerItems")?.as_array()?;
    let mut slots = Vec::new();
    for entry in arr {
        let slot = entry.get("slot").and_then(value_to_i64).unwrap_or(0);
        let item = entry.get("item").cloned().unwrap_or(Value::Null);
        slots.push(ContainerSlot { slot, item: parse_nbt_item(&item) });
    }
    Some(slots)
}

fn parse_sign_lines(v: &Value) -> Option<[String; 4]> {
    let arr = get(v, "signLines")?.as_array()?;
    let at = |i: usize| arr.get(i).and_then(Value::as_str).unwrap_or("").to_string();
    Some([at(0), at(1), at(2), at(3)])
}

fn build_setblock(form: &Value, version: GiveVersion) -> Result<String, String> {
    let nbt = if let Some(cb) = get(form, "commandBlock") {
        if cb.is_object() {
            SetblockNbt::CommandBlock(SetblockCommandBlockOptions {
                command: v_str_or(cb, "command", ""),
                auto: v_bool(cb, "auto"),
                track_output: v_bool_opt(cb, "trackOutput"),
            })
        } else {
            SetblockNbt::None
        }
    } else if let Some(items) = parse_container_items(form) {
        if items.is_empty() {
            SetblockNbt::None
        } else {
            SetblockNbt::ContainerItems(items)
        }
    } else if let Some(lines) = parse_sign_lines(form) {
        SetblockNbt::SignLines(lines)
    } else {
        SetblockNbt::None
    };

    let f = SetblockForm {
        version,
        with_slash: v_bool(form, "withSlash"),
        x: v_str_or(form, "x", ""),
        y: v_str_or(form, "y", ""),
        z: v_str_or(form, "z", ""),
        block: v_str_or(form, "block", ""),
        blockstate: v_str_owned(form, "blockstate"),
        mode: parse_setblock_mode(form),
        nbt,
    };
    Ok(build_setblock_command(&f))
}

fn parse_nbt_attribute(v: &Value) -> Option<NbtAttribute> {
    let id = v.get("id").and_then(Value::as_str)?.to_string();
    let base = v.get("base").and_then(Value::as_f64).unwrap_or(0.0);
    Some(NbtAttribute { id, base })
}

fn parse_nbt_effect(v: &Value) -> Option<NbtEffect> {
    let id = v.get("id").and_then(Value::as_str)?.to_string();
    Some(NbtEffect {
        id,
        duration: v.get("duration").and_then(value_to_i64),
        amplifier: v.get("amplifier").and_then(value_to_i64),
        show_particles: v.get("showParticles").map(truthy),
    })
}

fn parse_equipment(v: &Value) -> Option<NbtEquipment> {
    if !v.is_object() {
        return None;
    }
    let slot = |k: &str| v.get(k).map(parse_nbt_item);
    Some(NbtEquipment {
        mainhand: slot("mainhand"),
        offhand: slot("offhand"),
        head: slot("head"),
        chest: slot("chest"),
        legs: slot("legs"),
        feet: slot("feet"),
    })
}

fn parse_passenger(v: &Value) -> SummonPassenger {
    SummonPassenger {
        entity_type: v_str_or(v, "entityType", ""),
        no_ai: v_bool(v, "noAI"),
        silent: v_bool(v, "silent"),
        custom_name: get(v, "customName").map(parse_rich_line),
        extra_nbt: v_str_owned(v, "extraNbt"),
    }
}

fn build_summon(form: &Value, version: GiveVersion) -> Result<String, String> {
    let rotation = get(form, "rotation").and_then(Value::as_array).and_then(|a| {
        let yaw = a.first()?.as_f64()?;
        let pitch = a.get(1)?.as_f64()?;
        Some((yaw, pitch))
    });
    let tags: Vec<String> =
        v_arr(form, "tags").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    let attributes: Vec<NbtAttribute> =
        v_arr(form, "attributes").iter().filter_map(parse_nbt_attribute).collect();
    let effects: Vec<NbtEffect> = v_arr(form, "effects").iter().filter_map(parse_nbt_effect).collect();
    let equipment = get(form, "equipment").and_then(parse_equipment);
    let passengers: Vec<SummonPassenger> = v_arr(form, "passengers").iter().map(parse_passenger).collect();

    let f = SummonForm {
        version,
        with_slash: v_bool(form, "withSlash"),
        entity_type: v_str_or(form, "entityType", ""),
        x: v_str_owned(form, "x"),
        y: v_str_owned(form, "y"),
        z: v_str_owned(form, "z"),
        custom_name: get(form, "customName").map(parse_rich_line),
        no_ai: v_bool(form, "noAI"),
        silent: v_bool(form, "silent"),
        persistence_required: v_bool(form, "persistenceRequired"),
        invulnerable: v_bool(form, "invulnerable"),
        no_gravity: v_bool(form, "noGravity"),
        glowing: v_bool(form, "glowing"),
        rotation,
        health: v_f64(form, "health"),
        tags,
        attributes,
        effects,
        equipment,
        passengers,
        extra_nbt: v_str_owned(form, "extraNbt"),
    };
    Ok(build_summon_command(&f))
}

/// 把单条意图分派到对应构建器。version 为目标 Minecraft 版本。
/// 对应 `dispatch.ts::dispatchIntent`。
pub fn dispatch_intent(intent: CommandIntent, version: GiveVersion) -> DispatchResult {
    if let Some(catalog_error) = validate_intent_catalog(&intent, version) {
        return DispatchResult { intent, command: None, error: Some(catalog_error), r#loop: false };
    }

    let form = intent.form().clone();
    let is_loop = matches!(&intent, CommandIntent::Execute(_)) && v_bool(&form, "loop");

    let result: Result<String, String> = match &intent {
        CommandIntent::Give(f) => build_give(f, version),
        CommandIntent::Say(f) => build_say(f),
        CommandIntent::EffectGive(f) => build_effect_give(f),
        CommandIntent::EffectClear(f) => build_effect_clear(f),
        CommandIntent::Tp(f) => build_tp(f),
        CommandIntent::Setblock(f) => build_setblock(f, version),
        CommandIntent::Summon(f) => build_summon(f, version),
        CommandIntent::Fill(f) => build_fill(f),
        CommandIntent::Clone(f) => build_clone(f),
        CommandIntent::Enchant(f) => build_enchant(f),
        CommandIntent::Execute(f) => build_execute(f),
        CommandIntent::Scoreboard(f) => build_scoreboard(f),
        CommandIntent::Attribute(f) => build_attribute(f, version),
        CommandIntent::Particle(f) => build_particle(f),
        CommandIntent::Unknown(name, _) => Err(format!(
            "未知指令类型: {}",
            serde_json::to_string(&Value::String(name.clone())).unwrap_or_default()
        )),
    };

    match result {
        Ok(command) => DispatchResult { intent, command: Some(command), error: None, r#loop: is_loop },
        Err(err) => DispatchResult { intent, command: None, error: Some(err), r#loop: false },
    }
}

/// 批量分派。返回与输入顺序一致的结果数组。对应 `dispatch.ts::dispatchIntents`。
pub fn dispatch_intents(intents: Vec<CommandIntent>, version: GiveVersion) -> Vec<DispatchResult> {
    intents.into_iter().map(|intent| dispatch_intent(intent, version)).collect()
}

// =====================================================================================
// 测试
// =====================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: GiveVersion = GiveVersion::Java1_21_5;
    const MID: GiveVersion = GiveVersion::Java1_21_4;
    const BEDROCK: GiveVersion = GiveVersion::Bedrock;

    fn intent(command: &str, form: Value) -> CommandIntent {
        CommandIntent::new(command, form)
    }

    fn dispatch(command: &str, form: Value, version: GiveVersion) -> DispatchResult {
        dispatch_intent(intent(command, form), version)
    }

    // ---------------- dispatch 基本分派 ----------------

    #[test]
    fn say_intent_dispatches() {
        let r = dispatch("say", serde_json::json!({"message": "hi"}), MODERN);
        assert_eq!(r.command.as_deref(), Some("say hi"));
        assert_eq!(r.error, None);
    }

    #[test]
    fn give_intent_reuses_existing_give_builder() {
        let r = dispatch(
            "give",
            serde_json::json!({"item": "minecraft:mace", "target": "@s", "count": 1}),
            MODERN,
        );
        assert_eq!(r.command.as_deref(), Some("give @s minecraft:mace 1"));
    }

    #[test]
    fn version_injected_by_dispatcher_legacy_attributes() {
        let r = dispatch(
            "summon",
            serde_json::json!({"entityType": "zombie", "attributes": [{"id": "max_health", "base": 40}]}),
            MID,
        );
        assert_eq!(
            r.command.as_deref(),
            Some(r#"summon minecraft:zombie ~ ~ ~ {Attributes:[{Name:"minecraft:generic.max_health",Base:40d}]}"#)
        );
    }

    #[test]
    fn illegal_intent_is_captured_not_thrown() {
        let r = dispatch("execute", serde_json::json!({"subcommands": []}), MODERN);
        assert_eq!(r.command, None);
        assert!(r.error.as_deref().is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn batch_dispatch_keeps_order_and_length_and_isolates_failures() {
        let results = dispatch_intents(
            vec![
                intent("say", serde_json::json!({"message": "a"})),
                intent("execute", serde_json::json!({"subcommands": []})),
                intent("enchant", serde_json::json!({"targets": "@s", "enchantment": "sharpness"})),
            ],
            MODERN,
        );
        assert_eq!(results.len(), 3);
        assert_eq!(results[2].command.as_deref(), Some("enchant @s minecraft:sharpness"));
    }

    #[test]
    fn particle_intent_dispatches() {
        let r = dispatch("particle", serde_json::json!({"name": "flame", "count": 5}), MODERN);
        assert_eq!(r.command.as_deref(), Some("particle minecraft:flame ~ ~ ~ 0 0 0 1 5"));
    }

    #[test]
    fn execute_loop_true_propagates_to_dispatch_result() {
        let looped = dispatch(
            "execute",
            serde_json::json!({"subcommands": ["at @e[type=minecraft:arrow]"], "run": "kill @s", "loop": true}),
            MODERN,
        );
        assert_eq!(looped.r#loop, true);

        let not_looped =
            dispatch("execute", serde_json::json!({"subcommands": ["at @s"], "run": "say hi"}), MODERN);
        assert_eq!(not_looped.r#loop, false);

        let give_result = dispatch("give", serde_json::json!({"item": "minecraft:stone"}), MODERN);
        assert_eq!(give_result.r#loop, false);
    }

    #[test]
    fn unknown_command_produces_readable_error() {
        let r = dispatch("no_such_thing", serde_json::json!({}), MODERN);
        assert_eq!(r.error.as_deref(), Some(r#"未知指令类型: "no_such_thing""#));
    }

    // ---------------- 目录存在性校验（拦 AI 编造的 id） ----------------

    #[test]
    fn fabricated_item_is_blocked() {
        let fake = dispatch("give", serde_json::json!({"item": "minecraft:super_death_sword"}), MODERN);
        assert_eq!(fake.command, None);
        assert!(fake.error.as_deref().unwrap().contains("super_death_sword"));

        let real =
            dispatch("give", serde_json::json!({"item": "minecraft:diamond_sword", "target": "@s"}), MODERN);
        assert_eq!(real.command.as_deref(), Some("give @s minecraft:diamond_sword 1"));

        let bare_real = dispatch("give", serde_json::json!({"item": "diamond_sword", "target": "@s"}), MODERN);
        assert_eq!(bare_real.command.as_deref(), Some("give @s minecraft:diamond_sword 1"));

        let fake_enchant = dispatch(
            "give",
            serde_json::json!({"item": "minecraft:bow", "enchantments": [{"id": "minecraft:super_power", "level": 1}]}),
            MODERN,
        );
        assert_eq!(fake_enchant.command, None);
    }

    #[test]
    fn fabricated_block_is_blocked() {
        let fake_block = dispatch(
            "setblock",
            serde_json::json!({"x": "0", "y": "0", "z": "0", "block": "minecraft:fake_ore"}),
            MODERN,
        );
        assert_eq!(fake_block.command, None);

        let real_block = dispatch(
            "fill",
            serde_json::json!({"from": ["0", "0", "0"], "to": ["1", "1", "1"], "block": "stone"}),
            MODERN,
        );
        assert!(real_block.command.as_deref().unwrap().contains("minecraft:stone"));
    }

    #[test]
    fn fabricated_effect_attribute_enchant_are_blocked() {
        let fake_effect =
            dispatch("effect_give", serde_json::json!({"target": "@s", "effect": "minecraft:super_buff"}), MODERN);
        assert_eq!(fake_effect.command, None);

        let fake_attr = dispatch(
            "attribute",
            serde_json::json!({"target": "@s", "attribute": "minecraft:luckiness", "action": {"kind": "base_set", "value": 1}}),
            MODERN,
        );
        assert_eq!(fake_attr.command, None);

        let fake_enchant2 =
            dispatch("enchant", serde_json::json!({"targets": "@s", "enchantment": "minecraft:god_mode"}), MODERN);
        assert_eq!(fake_enchant2.command, None);
    }

    #[test]
    fn summon_format_and_catalog_checks() {
        let real_entity = dispatch("summon", serde_json::json!({"entityType": "zombie"}), MODERN);
        assert_eq!(real_entity.command.as_deref(), Some("summon minecraft:zombie"));

        let bad_format = dispatch("summon", serde_json::json!({"entityType": "not a valid id!!"}), MODERN);
        assert_eq!(bad_format.command, None);

        let fake_summon_effect = dispatch(
            "summon",
            serde_json::json!({"entityType": "zombie", "effects": [{"id": "minecraft:super_regen", "duration": 100}]}),
            MODERN,
        );
        assert_eq!(fake_summon_effect.command, None);

        let fake_equip = dispatch(
            "summon",
            serde_json::json!({"entityType": "zombie", "equipment": {"mainhand": {"id": "minecraft:excalibur"}}}),
            MODERN,
        );
        assert_eq!(fake_equip.command, None);

        let real_equip = dispatch(
            "summon",
            serde_json::json!({"entityType": "zombie", "equipment": {"mainhand": {"id": "minecraft:diamond_sword"}}}),
            MODERN,
        );
        assert!(real_equip.command.as_deref().unwrap().contains("minecraft:diamond_sword"));
    }

    // ---------------- 之前漏掉校验的字段 ----------------

    #[test]
    fn effect_clear_now_validated_like_effect_give() {
        let blocked =
            dispatch("effect_clear", serde_json::json!({"target": "@s", "effect": "minecraft:super_buff"}), MODERN);
        assert_eq!(blocked.command, None);

        let real_effect =
            dispatch("effect_clear", serde_json::json!({"target": "@s", "effect": "minecraft:speed"}), MODERN);
        assert!(real_effect.command.is_some());

        let omitted_effect = dispatch("effect_clear", serde_json::json!({"target": "@s"}), MODERN);
        assert!(omitted_effect.command.is_some());
    }

    #[test]
    fn summon_passengers_validated() {
        let blocked = dispatch(
            "summon",
            serde_json::json!({"entityType": "zombie", "passengers": [{"entityType": "minecraft:dragon_lord"}]}),
            MODERN,
        );
        assert_eq!(blocked.command, None);

        let real = dispatch(
            "summon",
            serde_json::json!({"entityType": "pig", "passengers": [{"entityType": "chicken"}]}),
            MODERN,
        );
        assert!(real.command.is_some());
    }

    #[test]
    fn setblock_container_items_validated() {
        let blocked = dispatch(
            "setblock",
            serde_json::json!({
                "x": "0", "y": "0", "z": "0", "block": "chest",
                "containerItems": [{"slot": 0, "item": {"id": "minecraft:excalibur"}}]
            }),
            MODERN,
        );
        assert_eq!(blocked.command, None);

        let real = dispatch(
            "setblock",
            serde_json::json!({
                "x": "0", "y": "0", "z": "0", "block": "chest",
                "containerItems": [{"slot": 0, "item": {"id": "diamond", "count": 5}}]
            }),
            MODERN,
        );
        assert!(real.command.is_some());
    }

    #[test]
    fn fill_and_clone_filter_blocks_validated() {
        let blocked = dispatch(
            "fill",
            serde_json::json!({
                "from": ["0", "0", "0"], "to": ["1", "1", "1"], "block": "stone",
                "replaceFilter": {"block": "minecraft:fakeore"}
            }),
            MODERN,
        );
        assert_eq!(blocked.command, None);

        let real = dispatch(
            "fill",
            serde_json::json!({
                "from": ["0", "0", "0"], "to": ["1", "1", "1"], "block": "air",
                "replaceFilter": {"block": "water"}
            }),
            MODERN,
        );
        assert!(real.command.is_some());

        let clone_blocked = dispatch(
            "clone",
            serde_json::json!({
                "begin": ["0", "0", "0"], "end": ["1", "1", "1"], "destination": ["5", "5", "5"],
                "maskMode": "filtered", "filter": {"block": "minecraft:fakeore"}
            }),
            MODERN,
        );
        assert_eq!(clone_blocked.command, None);
    }

    #[test]
    fn scoreboard_criteria_item_backed_validated() {
        let crit = |criteria: &str| {
            dispatch(
                "scoreboard",
                serde_json::json!({"action": {"kind": "objectives_add", "objective": "x", "criteria": criteria}}),
                MODERN,
            )
        };
        assert_eq!(crit("minecraft.used:minecraft.magic_wand").command, None);
        assert!(crit("minecraft.used:minecraft.fishing_rod").command.is_some());
        // custom 后面接的是统计项名不是物品，不该拿去查物品表
        assert!(crit("minecraft.custom:minecraft.sneak_time").command.is_some());
        assert!(crit("dummy").command.is_some());
    }

    // ---------------- 粒子目录校验 ----------------

    #[test]
    fn particle_catalog_validation() {
        let p = |name: &str| dispatch("particle", serde_json::json!({"name": name, "count": 5}), MODERN);

        assert_eq!(p("minecraft:sparkle_magic").command, None);
        assert_eq!(p("minecraft:flame").command.as_deref(), Some("particle minecraft:flame ~ ~ ~ 0 0 0 1 5"));
        assert_eq!(p("flame").command.as_deref(), Some("particle minecraft:flame ~ ~ ~ 0 0 0 1 5"));

        // 关键回归：参数化粒子的 {...} 附加数据不能参与查表，否则整类都会被误杀
        assert_eq!(
            p("minecraft:dust{color:[1.0,0.2,0.2],scale:1.5}").command.as_deref(),
            Some("particle minecraft:dust{color:[1.0,0.2,0.2],scale:1.5} ~ ~ ~ 0 0 0 1 5")
        );
        assert!(p(r#"minecraft:block{block_state:{Name:"minecraft:stone"}}"#).command.is_some());
        // 但花括号不该变成绕过校验的后门
        assert_eq!(p("minecraft:fakedust{color:[1,0,0]}").command, None);
    }

    // ---------------- 目录校验要跟着版本走（版本感知回归） ----------------
    //
    // 曾经的真 bug：校验集合在模块顶层用 Java 目录建好、不看 version，导致基岩版下
    // 两个方向同时错——真实的基岩 id 被当成"AI 编造"拦掉，Java id 反倒放行，
    // 拼出一条基岩里根本不存在的指令。

    #[test]
    fn catalog_validation_is_version_aware() {
        let web = dispatch("give", serde_json::json!({"item": "minecraft:web"}), BEDROCK);
        assert_eq!(web.command.as_deref(), Some("give @a web 1 0"));

        let cobweb_on_bedrock = dispatch("give", serde_json::json!({"item": "minecraft:cobweb"}), BEDROCK);
        assert_eq!(cobweb_on_bedrock.command, None);

        let bedrock_only = dispatch("give", serde_json::json!({"item": "minecraft:border_block"}), BEDROCK);
        assert_eq!(bedrock_only.command.as_deref(), Some("give @a border_block 1 0"));

        // 反向：Java 侧不能被这次改动带歪
        let cobweb_on_java = dispatch("give", serde_json::json!({"item": "minecraft:cobweb"}), MODERN);
        assert_eq!(cobweb_on_java.command.as_deref(), Some("give @a minecraft:cobweb 1"));

        let web_on_java = dispatch("give", serde_json::json!({"item": "minecraft:web"}), MODERN);
        assert_eq!(web_on_java.command, None);

        // 编造的 id 两个版本都要拦
        for version in [MODERN, BEDROCK] {
            let fake = dispatch("give", serde_json::json!({"item": "minecraft:excalibur_of_doom"}), version);
            assert_eq!(fake.command, None);
        }
    }

    // ---------------- 端到端：AI 意图 → 命令字符串 ----------------

    #[test]
    fn end_to_end_exploding_arrow() {
        let results = dispatch_intents(
            vec![
                intent(
                    "give",
                    serde_json::json!({"item": "minecraft:bow", "count": 1, "enchantments": [{"id": "minecraft:power", "level": 5}]}),
                ),
                intent(
                    "execute",
                    serde_json::json!({
                        "subcommands": ["at @e[type=minecraft:arrow,nbt={inGround:1b}]"],
                        "run": "summon minecraft:tnt ~ ~ ~ {fuse:0s}"
                    }),
                ),
            ],
            MODERN,
        );
        assert_eq!(results[0].command.as_deref(), Some("give @a minecraft:bow[enchantments={power:5}] 1"));
        assert_eq!(
            results[1].command.as_deref(),
            Some("execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}")
        );
    }

    #[test]
    fn end_to_end_landmine_command_block() {
        let r = dispatch(
            "setblock",
            serde_json::json!({
                "x": "~", "y": "~", "z": "~",
                "block": "repeating_command_block",
                "commandBlock": {
                    "command": "execute at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:\"minecraft:tnt\"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",
                    "auto": true
                }
            }),
            MODERN,
        );
        assert_eq!(
            r.command.as_deref(),
            Some(
                r#"setblock ~ ~ ~ minecraft:repeating_command_block{Command:"execute at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:\"minecraft:tnt\"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",auto:1b}"#
            )
        );
    }

    #[test]
    fn end_to_end_custom_data_marked_arrow() {
        let results = dispatch_intents(
            vec![
                intent("give", serde_json::json!({"item": "minecraft:bow", "count": 1})),
                intent(
                    "give",
                    serde_json::json!({"item": "minecraft:arrow", "count": 16, "customData": "{soul_tnt_arrow:1b}"}),
                ),
                intent(
                    "execute",
                    serde_json::json!({
                        "subcommands": ["at @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}]"],
                        "run": "summon minecraft:tnt ~ ~ ~ {fuse:0s}"
                    }),
                ),
            ],
            MODERN,
        );
        assert_eq!(results[0].command.as_deref(), Some("give @a minecraft:bow 1"));
        assert_eq!(
            results[1].command.as_deref(),
            Some("give @a minecraft:arrow[custom_data={soul_tnt_arrow:1b}] 16")
        );
        assert_eq!(
            results[2].command.as_deref(),
            Some("execute at @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}")
        );
    }

    #[test]
    fn end_to_end_zombie_with_health_attribute_and_enchanted_equipment() {
        let results = dispatch_intents(
            vec![intent(
                "summon",
                serde_json::json!({
                    "entityType": "minecraft:zombie",
                    "noAI": true,
                    "health": 40,
                    "attributes": [{"id": "max_health", "base": 40}],
                    "equipment": {
                        "mainhand": {"id": "minecraft:diamond_sword", "enchantments": [{"id": "minecraft:sharpness", "level": 5}]}
                    }
                }),
            )],
            MODERN,
        );
        assert_eq!(
            results[0].command.as_deref(),
            Some(
                r#"summon minecraft:zombie ~ ~ ~ {NoAI:1b,Health:40f,attributes:[{id:"minecraft:max_health",base:40d}],equipment:{mainhand:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{sharpness:5}}}}}"#
            )
        );
    }
}
