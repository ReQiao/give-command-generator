/**
 * give 命令语法探针集。
 *
 * 设计理念：对每个"特性"提供多个"候选格式"，由服务器判定哪种合法。
 * 这样不仅能验证 builder.ts 当前输出，还能主动发现某版本的正确写法。
 *
 * builderFamilies 标注 builder.ts 当前对哪些版本族会输出该格式：
 *   "legacy" = buildJava121Legacy  (java_1_21 / java_1_21_1)
 *   "modern" = buildJava12111Plus  (java_1_21_11_plus 及更新)
 *
 * 命令一律不带前导斜杠（RCON 要求），统一使用 minecraft:stone 与合法 id，
 * 这样失败只可能来自组件结构本身，而非物品/附魔不存在。
 */

const g = (component) => `give @a minecraft:stone[${component}] 1`;
// 用于耐久相关组件：石头可堆叠会触发 "damageable and stackable" 物品约束错误，
// 因此这类探针改用本身不可堆叠的物品，确保失败只来自语法本身。
const gd = (component) => `give @a minecraft:diamond_sword[${component}] 1`;

/** 版本 -> builder 版本族。1.21/1.21.1 为 legacy，其余 Java 版默认 modern。 */
export function familyOf(version) {
  if (version === "1.21" || version === "1.21.1") return "legacy";
  return "modern";
}

export const PROBES = [
  // ---- 基础 ----
  { feature: "basic", id: "basic_plain", command: "give @a minecraft:stone 1", builderFamilies: ["legacy", "modern"], note: "最基础的 give" },

  // ---- 文本：custom_name ----
  { feature: "custom_name", id: "custom_name_json", command: g('custom_name=[{"text":"hi"}]'), builderFamilies: ["modern"], note: "直接 JSON 数组（紧凑）" },
  { feature: "custom_name", id: "custom_name_snbt_string", command: g(`custom_name='[{"text":"hi"}]'`), builderFamilies: ["legacy"], note: "SNBT 单引号字符串" },

  // ---- 文本：item_name ----
  { feature: "item_name", id: "item_name_json", command: g('item_name=[{"text":"hi"}]'), builderFamilies: ["modern"], note: "直接 JSON 数组" },
  { feature: "item_name", id: "item_name_snbt_string", command: g(`item_name='[{"text":"hi"}]'`), builderFamilies: ["legacy"], note: "SNBT 单引号字符串" },

  // ---- 文本：lore ----
  { feature: "lore", id: "lore_json_arrays", command: g('lore=[[{"text":"a"}],[{"text":"b"}]]'), builderFamilies: ["modern"], note: "数组的数组（直接 JSON）" },
  { feature: "lore", id: "lore_snbt_strings", command: g(`lore=['[{"text":"a"}]','[{"text":"b"}]']`), builderFamilies: ["legacy"], note: "字符串数组（每项单引号 SNBT）" },

  // ---- rarity / glint ----
  { feature: "rarity", id: "rarity_epic", command: g("rarity=epic"), builderFamilies: ["legacy", "modern"], note: "稀有度" },
  { feature: "enchant_glint", id: "glint_true", command: g("enchantment_glint_override=true"), builderFamilies: ["legacy", "modern"], note: "发光覆盖" },

  // ---- enchantments ----
  { feature: "enchantments", id: "ench_levels", command: g("enchantments={levels:{unbreaking:1}}"), builderFamilies: ["legacy"], note: "带 levels 外层" },
  { feature: "enchantments", id: "ench_flat", command: g("enchantments={unbreaking:1}"), builderFamilies: ["modern"], note: "扁平，无 levels 外层" },

  // ---- attribute_modifiers ----
  { feature: "attribute_modifiers", id: "attr_array_type_unquoted_plain", command: g('attribute_modifiers=[{type:armor,amount:1,id:"test:x",operation:add_value}]'), builderFamilies: ["modern"], note: "数组形式，type 不带引号/无 generic 前缀（builder modern）" },
  { feature: "attribute_modifiers", id: "attr_array_type_quoted_generic", command: g('attribute_modifiers=[{type:"generic.armor",amount:1,id:"test:x",operation:add_value}]'), builderFamilies: [], note: "数组形式，type 带引号且含 generic. 前缀" },
  { feature: "attribute_modifiers", id: "attr_wrapper_type_quoted_generic", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:"test:x",operation:add_value}]}'), builderFamilies: ["legacy"], note: "modifiers 外层 + 引号 generic.（builder legacy）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_id_numeric", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:123,operation:add_value}]}'), builderFamilies: [], note: "id 为纯数字（服务器拒绝，builder 已改为始终引号）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_id_quoted_number", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:"123",operation:add_value}]}'), builderFamilies: [], note: "id 为带引号数字字符串（探查修复方向）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_with_slot", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,slot:mainhand,id:"test:x",operation:add_value}]}'), builderFamilies: [], note: "带 slot 字段" },

  // ---- unbreakable ----
  { feature: "unbreakable", id: "unbreakable_empty", command: g("unbreakable={}"), builderFamilies: ["legacy", "modern"], note: "空对象" },

  // ---- 数值类 ----
  { feature: "damage", id: "damage_int", command: gd("damage=1"), builderFamilies: ["legacy", "modern"], note: "已损耗值（不可堆叠物品）" },
  { feature: "max_damage", id: "max_damage_int", command: gd("max_damage=100"), builderFamilies: ["legacy", "modern"], note: "最大耐久（不可堆叠物品，避免 damageable+stackable 约束）" },
  { feature: "max_stack_size", id: "max_stack_size_int", command: g("max_stack_size=16"), builderFamilies: ["legacy", "modern"], note: "最大堆叠" },
  { feature: "repair_cost", id: "repair_cost_int", command: g("repair_cost=3"), builderFamilies: ["legacy", "modern"], note: "修复经验消耗" },

  // ---- food ----
  { feature: "food", id: "food_basic", command: g("food={nutrition:5,saturation:6}"), builderFamilies: ["legacy", "modern"], note: "基础营养/饱和" },
  { feature: "food", id: "food_can_always_eat", command: g("food={nutrition:5,saturation:6,can_always_eat:true}"), builderFamilies: ["legacy", "modern"], note: "can_always_eat（布尔）" },
  { feature: "food", id: "food_can_always_eat_byte", command: g("food={nutrition:5,saturation:6,can_always_eat:1b}"), builderFamilies: ["legacy", "modern"], note: "can_always_eat（1b 字节，builder 实际输出）" },
  { feature: "food", id: "food_eat_seconds", command: g("food={nutrition:5,saturation:6,eat_seconds:2}"), builderFamilies: ["legacy"], note: "eat_seconds 并入 food（legacy）" },
  { feature: "food", id: "food_effects_inside", command: g("food={nutrition:5,saturation:6,effects:[{effect:{id:speed,duration:20,amplifier:0},probability:1.0f}]}"), builderFamilies: ["legacy"], note: "效果并入 food.effects（legacy）" },

  // ---- consumable（modern，1.21.2+）----
  { feature: "consumable", id: "consumable_seconds", command: g("consumable={consume_seconds:2}"), builderFamilies: ["modern"], note: "独立 consumable" },
  { feature: "consumable", id: "consumable_on_consume_effects", command: g('consumable={on_consume_effects:[{type:"minecraft:apply_effects",effects:[{id:speed,duration:20,amplifier:0}],probability:1.0}]}'), builderFamilies: ["modern"], note: "on_consume_effects" },

  // ---- tool ----
  { feature: "tool", id: "tool_rules_blocks_stripped", command: g("tool={rules:[{blocks:[stone],speed:1.0,correct_for_drops:true}]}"), builderFamilies: [], note: "blocks 去命名空间，speed 浮点，correct_for_drops 布尔" },
  { feature: "tool", id: "tool_blocks_namespaced", command: g("tool={rules:[{blocks:[minecraft:stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: [], note: "blocks 带命名空间（探查命名空间是否被接受）" },
  { feature: "tool", id: "tool_rules_f_byte", command: g("tool={rules:[{blocks:[stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: ["legacy", "modern"], note: "speed 带 f 后缀、correct_for_drops 1b（builder 实际输出）" },
  { feature: "tool", id: "tool_full", command: g("tool={default_mining_speed:1.0,damage_per_block:1,rules:[{blocks:[stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: ["legacy", "modern"], note: "含 default_mining_speed/damage_per_block（blocks 去命名空间）" },

  // ---- can_place_on / can_break（CLAUDE.md 标记待确认）----
  { feature: "can_place_on", id: "can_place_on_blocks_obj", command: g("can_place_on=[{blocks:minecraft:stone}]"), builderFamilies: [], note: "无引号 blocks（两族均拒绝）" },
  { feature: "can_place_on", id: "can_place_on_predicates", command: g('can_place_on={predicates:[{blocks:"minecraft:stone"}]}'), builderFamilies: ["legacy"], note: "{predicates:[...]}（legacy 正确格式）" },
  { feature: "can_place_on", id: "can_place_on_predicates_stripped", command: g('can_place_on={predicates:[{blocks:"stone"}]}'), builderFamilies: [], note: "{predicates:[...]} 去命名空间" },
  { feature: "can_place_on", id: "can_place_on_predicates_multi", command: g('can_place_on={predicates:[{blocks:"minecraft:stone"},{blocks:"minecraft:dirt"}]}'), builderFamilies: [], note: "多个 predicate" },
  { feature: "can_place_on", id: "can_place_on_quoted_array", command: g('can_place_on=[{blocks:"minecraft:stone"}]'), builderFamilies: ["modern"], note: "直接列表 + 引号 blocks（modern 正确格式）" },
  { feature: "can_break", id: "can_break_blocks_obj", command: g("can_break=[{blocks:minecraft:stone}]"), builderFamilies: [], note: "无引号 blocks（两族均拒绝）" },
  { feature: "can_break", id: "can_break_predicates", command: g('can_break={predicates:[{blocks:"minecraft:stone"}]}'), builderFamilies: ["legacy"], note: "{predicates:[...]}（legacy 正确格式）" },
  { feature: "can_break", id: "can_break_quoted_array", command: g('can_break=[{blocks:"minecraft:stone"}]'), builderFamilies: ["modern"], note: "直接列表 + 引号 blocks（modern 正确格式）" },

  // ---- glider（modern，1.21.2+）----
  { feature: "glider", id: "glider_empty", command: g("glider={}"), builderFamilies: ["modern"], note: "鞘翅滑翔" },

  // ---- death_protection（modern）----
  { feature: "death_protection", id: "death_protection_empty", command: g("death_protection={}"), builderFamilies: ["modern"], note: "图腾式死亡保护（空）" },
  { feature: "death_protection", id: "death_protection_effects", command: g('death_protection={death_effects:[{type:"minecraft:apply_effects",effects:[{id:speed,duration:20,amplifier:0}],probability:1.0}]}'), builderFamilies: ["modern"], note: "带 death_effects" },

  // ---- tooltip_display（modern）----
  { feature: "tooltip_display", id: "tooltip_hidden", command: g("tooltip_display={hidden_components:[minecraft:enchantments]}"), builderFamilies: [], note: "隐藏组件（无引号，服务器拒绝）" },
  { feature: "tooltip_display", id: "tooltip_hidden_quoted", command: g('tooltip_display={hidden_components:["minecraft:enchantments"]}'), builderFamilies: ["modern"], note: "隐藏组件（引号列表，modern 正确格式）" },
  { feature: "tooltip_display", id: "tooltip_hide_bool", command: g("tooltip_display={hide_tooltip:true}"), builderFamilies: [], note: "hide_tooltip 布尔" },
];

/**
 * 把响应文本分类为 valid / invalid / unknown。
 * - 语法合法但无玩家：包含 "No player was found"
 * - 语法合法且执行：包含 "Gave"/"Given"
 * - 语法非法：包含 "<--[HERE]" 错误指示，或常见报错关键词
 */
export function classifyResponse(response) {
  const r = (response || "").trim();
  if (!r) return "unknown";
  if (/No player was found|That player does not exist/i.test(r)) return "valid";
  if (/\bGave\b|\bGiven\b/i.test(r)) return "valid";
  if (/<--\[HERE\]/.test(r)) return "invalid";
  if (/Unknown|Expected|Incorrect|Unexpected|Invalid|Failed to|Could not|cannot|can't|not allowed|Error/i.test(r)) {
    return "invalid";
  }
  return "unknown";
}
