/**
 * Minecraft 指令语法探针集（give + P1 其它指令）。
 *
 * 设计理念：对每个"特性"提供多个"候选格式"，由服务器判定哪种合法。
 * 这样不仅能验证 builder.ts 当前输出，还能主动发现某版本的正确写法。
 *
 * 两类探针：
 *   1. give 探针 —— 与 builder.ts 输出对照（report 给 PASS/FAIL）。
 *   2. 其它指令探针（say/tp/effect/setblock/summon）—— builder 尚未产出，
 *      属纯语法调查（report 给 valid/invalid/unknown 真值表）。
 *
 * builderFamilies 标注 builder.ts（或拟实现的 builder）对哪些版本族输出该格式：
 *   "early"  = java_1_20_5 / java_1_20_6
 *   "legacy" = java_1_21 / java_1_21_1
 *   "mid"    = java_1_21_2 / java_1_21_3 / java_1_21_4
 *   "modern" = java_1_21_5 及更新
 *
 * 命令一律不带前导斜杠（RCON 要求），统一使用合法 id 与相对/本地坐标，
 * 这样失败只可能来自语法结构本身，而非物品/实体/坐标不存在。
 */

const g = (component) => `give @a minecraft:stone[${component}] 1`;
// 用于耐久相关组件：石头可堆叠会触发 "damageable and stackable" 物品约束错误，
// 因此这类探针改用本身不可堆叠的物品，确保失败只来自语法本身。
const gd = (component) => `give @a minecraft:diamond_sword[${component}] 1`;

/** 版本 -> builder 版本族。 */
export function familyOf(version) {
  if (version === "1.21" || version === "1.21.1") return "legacy";
  if (version === "1.20.5" || version === "1.20.6") return "early";
  if (version === "1.21.2" || version === "1.21.3" || version === "1.21.4") return "mid";
  return "modern";
}

export const PROBES = [
  // ---- 基础 ----
  { feature: "basic", id: "basic_plain", command: "give @a minecraft:stone 1", builderFamilies: ["early", "legacy", "mid", "modern"], note: "最基础的 give" },

  // ---- 文本：custom_name ----
  { feature: "custom_name", id: "custom_name_json", command: g('custom_name=[{"text":"hi"}]'), builderFamilies: ["modern"], note: "直接 JSON 数组（紧凑）" },
  { feature: "custom_name", id: "custom_name_snbt_string", command: g(`custom_name='[{"text":"hi"}]'`), builderFamilies: ["early", "legacy", "mid"], note: "SNBT 单引号字符串" },

  // ---- 文本：item_name ----
  { feature: "item_name", id: "item_name_json", command: g('item_name=[{"text":"hi"}]'), builderFamilies: ["modern"], note: "直接 JSON 数组" },
  { feature: "item_name", id: "item_name_snbt_string", command: g(`item_name='[{"text":"hi"}]'`), builderFamilies: ["early", "legacy", "mid"], note: "SNBT 单引号字符串" },

  // ---- 文本：lore ----
  { feature: "lore", id: "lore_json_arrays", command: g('lore=[[{"text":"a"}],[{"text":"b"}]]'), builderFamilies: ["modern"], note: "数组的数组（直接 JSON）" },
  { feature: "lore", id: "lore_snbt_strings", command: g(`lore=['[{"text":"a"}]','[{"text":"b"}]']`), builderFamilies: ["early", "legacy", "mid"], note: "字符串数组（每项单引号 SNBT）" },

  // ---- rarity / glint ----
  { feature: "rarity", id: "rarity_epic", command: g("rarity=epic"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "稀有度" },
  { feature: "enchant_glint", id: "glint_true", command: g("enchantment_glint_override=true"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "发光覆盖" },

  // ---- enchantments ----
  { feature: "enchantments", id: "ench_levels", command: g("enchantments={levels:{unbreaking:1}}"), builderFamilies: ["legacy"], note: "带 levels 外层" },
  { feature: "enchantments", id: "ench_flat", command: g("enchantments={unbreaking:1}"), builderFamilies: ["early", "mid", "modern"], note: "扁平，无 levels 外层" },

  // ---- attribute_modifiers ----
  { feature: "attribute_modifiers", id: "attr_array_type_unquoted_plain", command: g('attribute_modifiers=[{type:armor,amount:1,id:"test:x",operation:add_value}]'), builderFamilies: ["mid", "modern"], note: "数组形式，type 不带引号/无 generic 前缀（builder modern/mid）" },
  { feature: "attribute_modifiers", id: "attr_array_type_quoted_generic", command: g('attribute_modifiers=[{type:"generic.armor",amount:1,id:"test:x",operation:add_value}]'), builderFamilies: [], note: "数组形式，type 带引号且含 generic. 前缀" },
  { feature: "attribute_modifiers", id: "attr_array_type_ns_quoted", command: g('attribute_modifiers=[{type:"minecraft:generic.armor",amount:1,id:"test:x",operation:add_value}]'), builderFamilies: [], note: "数组形式，type 带 minecraft: 命名空间前缀（1.20.5 候选）" },
  { feature: "attribute_modifiers", id: "attr_array_type_ns_unquoted", command: g('attribute_modifiers=[{type:minecraft:generic.armor,amount:1,id:"test:x",operation:add_value}]'), builderFamilies: [], note: "数组形式，type 不带引号但带 minecraft: 前缀（1.20.5 候选）" },
  { feature: "attribute_modifiers", id: "attr_array_op_quoted", command: g('attribute_modifiers=[{type:armor,amount:1,id:"test:x",operation:"add_value"}]'), builderFamilies: [], note: "operation 用引号字符串（1.20.5 候选）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_type_quoted_generic", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:"test:x",operation:add_value}]}'), builderFamilies: ["legacy"], note: "modifiers 外层 + 引号 generic.（builder legacy）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_id_numeric", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:123,operation:add_value}]}'), builderFamilies: [], note: "id 为纯数字（服务器拒绝，builder 已改为始终引号）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_id_quoted_number", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,id:"123",operation:add_value}]}'), builderFamilies: [], note: "id 为带引号数字字符串（探查修复方向）" },
  { feature: "attribute_modifiers", id: "attr_wrapper_with_slot", command: g('attribute_modifiers={modifiers:[{type:"generic.armor",amount:1,slot:mainhand,id:"test:x",operation:add_value}]}'), builderFamilies: [], note: "带 slot 字段" },

  // ---- unbreakable ----
  { feature: "unbreakable", id: "unbreakable_empty", command: g("unbreakable={}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "空对象" },

  // ---- 数值类 ----
  { feature: "damage", id: "damage_int", command: gd("damage=1"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "已损耗值（不可堆叠物品）" },
  { feature: "max_damage", id: "max_damage_int", command: gd("max_damage=100"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "最大耐久（不可堆叠物品，避免 damageable+stackable 约束）" },
  { feature: "max_stack_size", id: "max_stack_size_int", command: g("max_stack_size=16"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "最大堆叠" },
  { feature: "repair_cost", id: "repair_cost_int", command: g("repair_cost=3"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "修复经验消耗" },

  // ---- food ----
  { feature: "food", id: "food_basic", command: g("food={nutrition:5,saturation:6}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "基础营养/饱和" },
  { feature: "food", id: "food_can_always_eat", command: g("food={nutrition:5,saturation:6,can_always_eat:true}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "can_always_eat（布尔）" },
  { feature: "food", id: "food_can_always_eat_byte", command: g("food={nutrition:5,saturation:6,can_always_eat:1b}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "can_always_eat（1b 字节，builder 实际输出）" },
  { feature: "food", id: "food_eat_seconds", command: g("food={nutrition:5,saturation:6,eat_seconds:2}"), builderFamilies: ["legacy"], note: "eat_seconds 并入 food（legacy）" },
  { feature: "food", id: "food_effects_inside", command: g("food={nutrition:5,saturation:6,effects:[{effect:{id:speed,duration:20,amplifier:0},probability:1.0f}]}"), builderFamilies: ["legacy"], note: "效果并入 food.effects（legacy）" },

  // ---- consumable（modern，1.21.2+）----
  { feature: "consumable", id: "consumable_seconds", command: g("consumable={consume_seconds:2}"), builderFamilies: ["mid", "modern"], note: "独立 consumable" },
  { feature: "consumable", id: "consumable_on_consume_effects", command: g('consumable={on_consume_effects:[{type:"minecraft:apply_effects",effects:[{id:speed,duration:20,amplifier:0}],probability:1.0}]}'), builderFamilies: ["mid", "modern"], note: "on_consume_effects" },
  { feature: "consumable", id: "consumable_sound_particles", command: g('consumable={consume_seconds:2,sound:"minecraft:entity.generic.eat",has_consume_particles:1b}'), builderFamilies: ["mid", "modern"], note: "sound + has_consume_particles（builder 实际输出字段）" },

  // ---- tool ----
  { feature: "tool", id: "tool_rules_blocks_stripped", command: g("tool={rules:[{blocks:[stone],speed:1.0,correct_for_drops:true}]}"), builderFamilies: [], note: "blocks 去命名空间，speed 浮点，correct_for_drops 布尔" },
  { feature: "tool", id: "tool_blocks_namespaced", command: g("tool={rules:[{blocks:[minecraft:stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: [], note: "blocks 带命名空间（探查命名空间是否被接受）" },
  { feature: "tool", id: "tool_rules_f_byte", command: g("tool={rules:[{blocks:[stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "speed 带 f 后缀、correct_for_drops 1b（builder 实际输出）" },
  { feature: "tool", id: "tool_full", command: g("tool={default_mining_speed:1.0,damage_per_block:1,rules:[{blocks:[stone],speed:1.0f,correct_for_drops:1b}]}"), builderFamilies: ["early", "legacy", "mid", "modern"], note: "含 default_mining_speed/damage_per_block（blocks 去命名空间）" },

  // ---- can_place_on / can_break（CLAUDE.md 标记待确认）----
  { feature: "can_place_on", id: "can_place_on_blocks_obj", command: g("can_place_on=[{blocks:minecraft:stone}]"), builderFamilies: [], note: "无引号 blocks（两族均拒绝）" },
  { feature: "can_place_on", id: "can_place_on_predicates", command: g('can_place_on={predicates:[{blocks:"minecraft:stone"}]}'), builderFamilies: ["early", "legacy", "mid"], note: "{predicates:[...]}（legacy/mid/early 正确格式）" },
  { feature: "can_place_on", id: "can_place_on_predicates_stripped", command: g('can_place_on={predicates:[{blocks:"stone"}]}'), builderFamilies: [], note: "{predicates:[...]} 去命名空间" },
  { feature: "can_place_on", id: "can_place_on_predicates_multi", command: g('can_place_on={predicates:[{blocks:"minecraft:stone"},{blocks:"minecraft:dirt"}]}'), builderFamilies: [], note: "多个 predicate" },
  { feature: "can_place_on", id: "can_place_on_quoted_array", command: g('can_place_on=[{blocks:"minecraft:stone"}]'), builderFamilies: ["modern"], note: "直接列表 + 引号 blocks（modern 正确格式）" },
  { feature: "can_break", id: "can_break_blocks_obj", command: g("can_break=[{blocks:minecraft:stone}]"), builderFamilies: [], note: "无引号 blocks（两族均拒绝）" },
  { feature: "can_break", id: "can_break_predicates", command: g('can_break={predicates:[{blocks:"minecraft:stone"}]}'), builderFamilies: ["early", "legacy", "mid"], note: "{predicates:[...]}（legacy/mid/early 正确格式）" },
  { feature: "can_break", id: "can_break_quoted_array", command: g('can_break=[{blocks:"minecraft:stone"}]'), builderFamilies: ["modern"], note: "直接列表 + 引号 blocks（modern 正确格式）" },

  // ---- glider（modern，1.21.2+）----
  { feature: "glider", id: "glider_empty", command: g("glider={}"), builderFamilies: ["mid", "modern"], note: "鞘翅滑翔" },

  // ---- death_protection（modern）----
  { feature: "death_protection", id: "death_protection_empty", command: g("death_protection={}"), builderFamilies: ["mid", "modern"], note: "图腾式死亡保护（空）" },
  { feature: "death_protection", id: "death_protection_effects", command: g('death_protection={death_effects:[{type:"minecraft:apply_effects",effects:[{id:speed,duration:20,amplifier:0}],probability:1.0}]}'), builderFamilies: ["mid", "modern"], note: "带 death_effects" },

  // ---- tooltip_display（modern）----
  { feature: "tooltip_display", id: "tooltip_hidden", command: g("tooltip_display={hidden_components:[minecraft:enchantments]}"), builderFamilies: [], note: "隐藏组件（无引号，服务器拒绝）" },
  { feature: "tooltip_display", id: "tooltip_hidden_quoted", command: g('tooltip_display={hidden_components:["minecraft:enchantments"]}'), builderFamilies: ["modern"], note: "隐藏组件（引号列表，modern 正确格式）" },
  { feature: "tooltip_display", id: "tooltip_hide_bool", command: g("tooltip_display={hide_tooltip:true}"), builderFamilies: [], note: "hide_tooltip 布尔" },

  // ============================================================
  // P1 其它指令语法调查（builder 尚未产出，先建真值表）
  // 核心：/setblock（方块实体 NBT）与 /summon（实体 NBT）共用 SNBT {…} 语法，
  // 其中嵌套物品统一为 {id,count,components:{…}}，只需实现一次序列化器即可复用。
  // 坐标统一用相对/本地坐标，仅判定语法不依赖已加载区块。
  // ============================================================

  // ---- /say（纯文本，全版本一致）----
  { feature: "say", id: "say_basic", command: "say hello world", builderFamilies: ["early", "legacy", "mid", "modern"], note: "广播纯文本" },
  { feature: "say", id: "say_selector", command: "say @a", builderFamilies: ["early", "legacy", "mid", "modern"], note: "消息含选择器（解析为玩家名）" },

  // ---- /tp（坐标 / 旋转 / 朝向 / 目标）----
  { feature: "tp", id: "tp_coords_abs", command: "tp @s 0 64 0", builderFamilies: ["early", "legacy", "mid", "modern"], note: "绝对坐标" },
  { feature: "tp", id: "tp_coords_rel", command: "tp @s ~ ~ ~", builderFamilies: ["early", "legacy", "mid", "modern"], note: "相对坐标 ~" },
  { feature: "tp", id: "tp_coords_local", command: "tp @s ^ ^ ^1", builderFamilies: ["early", "legacy", "mid", "modern"], note: "本地坐标 ^" },
  { feature: "tp", id: "tp_rotation", command: "tp @s 0 64 0 90 45", builderFamilies: ["early", "legacy", "mid", "modern"], note: "带 yaw/pitch 旋转" },
  { feature: "tp", id: "tp_facing", command: "tp @s 0 64 0 facing 10 70 10", builderFamilies: ["early", "legacy", "mid", "modern"], note: "facing 朝向坐标" },
  { feature: "tp", id: "tp_to_entity", command: "tp @s @e[type=pig,limit=1]", builderFamilies: ["early", "legacy", "mid", "modern"], note: "传送到实体选择器" },
  { feature: "teleport", id: "teleport_alias", command: "teleport @s 0 64 0", builderFamilies: ["early", "legacy", "mid", "modern"], note: "teleport 别名" },

  // ---- /effect（give / clear）----
  { feature: "effect_give", id: "effect_give_basic", command: "effect give @a minecraft:speed", builderFamilies: ["early", "legacy", "mid", "modern"], note: "最简：仅效果 id" },
  { feature: "effect_give", id: "effect_give_seconds", command: "effect give @a minecraft:speed 30", builderFamilies: ["early", "legacy", "mid", "modern"], note: "时长（秒）" },
  { feature: "effect_give", id: "effect_give_amplifier", command: "effect give @a minecraft:speed 30 2", builderFamilies: ["early", "legacy", "mid", "modern"], note: "等级（amplifier）" },
  { feature: "effect_give", id: "effect_give_hide", command: "effect give @a minecraft:speed 30 2 true", builderFamilies: ["early", "legacy", "mid", "modern"], note: "隐藏粒子" },
  { feature: "effect_give", id: "effect_give_infinite", command: "effect give @a minecraft:speed infinite 1", builderFamilies: ["early", "legacy", "mid", "modern"], note: "无限时长（1.19.4+）" },
  { feature: "effect_clear", id: "effect_clear_all", command: "effect clear @a", builderFamilies: ["early", "legacy", "mid", "modern"], note: "清除全部效果" },
  { feature: "effect_clear", id: "effect_clear_one", command: "effect clear @a minecraft:speed", builderFamilies: ["early", "legacy", "mid", "modern"], note: "清除指定效果" },

  // ---- /setblock（方块状态 + 方块实体 NBT）----
  { feature: "setblock", id: "setblock_basic", command: "setblock ~ ~ ~ minecraft:stone", builderFamilies: ["early", "legacy", "mid", "modern"], note: "基础" },
  { feature: "setblock", id: "setblock_blockstate", command: "setblock ~ ~ ~ minecraft:oak_log[axis=x]", builderFamilies: ["early", "legacy", "mid", "modern"], note: "方块状态 [axis=x]" },
  { feature: "setblock", id: "setblock_mode_keep", command: "setblock ~ ~ ~ minecraft:stone keep", builderFamilies: ["early", "legacy", "mid", "modern"], note: "放置模式 keep" },
  { feature: "setblock_nbt", id: "setblock_nbt_commandblock", command: `setblock ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi",auto:1b}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "命令方块 NBT（方块实体，含 Command）" },
  { feature: "setblock_nbt", id: "setblock_nbt_container", command: `setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:1}]}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "★item-in-NBT（1.20.5+ 小写 count）" },
  { feature: "setblock_nbt", id: "setblock_nbt_container_components", command: `setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:stone",count:1,components:{"minecraft:enchantment_glint_override":true}}]}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "★完整 item-in-NBT（含 components，用非文本组件避免序列化分歧）" },

  // ---- /summon（实体 NBT）----
  { feature: "summon", id: "summon_basic", command: "summon minecraft:pig", builderFamilies: ["early", "legacy", "mid", "modern"], note: "最简：仅实体 id" },
  { feature: "summon", id: "summon_coords", command: "summon minecraft:pig 0 64 0", builderFamilies: ["early", "legacy", "mid", "modern"], note: "绝对坐标" },
  { feature: "summon", id: "summon_relative", command: "summon minecraft:pig ~ ~ ~", builderFamilies: ["early", "legacy", "mid", "modern"], note: "相对坐标" },
  { feature: "summon_nbt", id: "summon_nbt_flags", command: `summon minecraft:zombie ~ ~ ~ {NoAI:1b,Silent:1b,PersistenceRequired:1b}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "布尔字节标志" },
  { feature: "summon_nbt", id: "summon_nbt_customname_snbt", command: `summon minecraft:zombie ~ ~ ~ {CustomName:'{"text":"Boss"}'}`, builderFamilies: ["early", "legacy", "mid"], note: "CustomName SNBT 字符串（文本同 give，early/legacy/mid）" },
  { feature: "summon_nbt", id: "summon_nbt_customname_json", command: `summon minecraft:zombie ~ ~ ~ {CustomName:{"text":"Boss"}}`, builderFamilies: ["modern"], note: "CustomName 裸 JSON（modern 候选）" },
  { feature: "summon_nbt", id: "summon_nbt_handitems", command: `summon minecraft:zombie ~ ~ ~ {HandItems:[{id:"minecraft:diamond_sword",count:1},{}]}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "★HandItems 复用 item-in-NBT" },
  { feature: "summon_nbt", id: "summon_nbt_passenger", command: `summon minecraft:zombie ~ ~ ~ {Passengers:[{id:"minecraft:chicken"}]}`, builderFamilies: ["early", "legacy", "mid", "modern"], note: "嵌套实体 Passengers" },

  // ---- 版本敏感点：新旧两套 NBT 键变体 ----
  // ⚠ 实测发现：MC 对实体 NBT 的"未知键"是静默忽略而非报错，所以新旧变体在
  //   1.20.6 与 1.21.5 上"都"判为 valid（仅代表能解析，不代表键被采用）。
  //   要真正裁决哪套键有效，需语义验证：summon 后用 /data get entity 读回。
  //   故此处仅作"解析层"记录，builderFamilies 留空，留待后续语义探针。
  { feature: "summon_attributes", id: "summon_attr_new", command: `summon minecraft:zombie ~ ~ ~ {attributes:[{id:"minecraft:max_health",base:40}]}`, builderFamilies: [], note: "新属性 NBT（小写 attributes/id/base）—— 需语义验证" },
  { feature: "summon_attributes", id: "summon_attr_old", command: `summon minecraft:zombie ~ ~ ~ {Attributes:[{Name:"minecraft:generic.max_health",Base:40}]}`, builderFamilies: [], note: "旧属性 NBT（Attributes/Name/Base）—— 需语义验证" },
  { feature: "summon_effects", id: "summon_effects_new", command: `summon minecraft:zombie ~ ~ ~ {active_effects:[{id:"minecraft:speed",duration:200,amplifier:1}]}`, builderFamilies: [], note: "新效果 NBT（active_effects 字符串 id）—— 需语义验证" },
  { feature: "summon_effects", id: "summon_effects_old", command: `summon minecraft:zombie ~ ~ ~ {ActiveEffects:[{Id:1,Duration:200,Amplifier:1}]}`, builderFamilies: [], note: "旧效果 NBT（ActiveEffects/数字 Id）—— 需语义验证" },
];

/**
 * 把响应文本分类为 valid / invalid（跨指令通用，面向"语法"而非"运行结果"）。
 *
 * 核心原理：Minecraft 服务器先用 Brigadier 解析语法，再执行命令。
 *   - 语法非法 → 解析阶段报错，错误文本一律带 "<--[HERE]" 位置标记。
 *   - 语法合法 → 进入执行阶段，无论成功还是运行期失败，都说明语法已被接受。
 *
 * 因此判定只看是否解析失败，不看运行结果。以下都属"语法合法"：
 *   - 成功：    "Gave ...", "Summoned new ...", "Changed the block ..."
 *   - 空响应：  /say 等无回显
 *   - 运行期失败（与语法无关）：
 *               "No player was found", "No entity was found", "Could not set the block"
 *
 * 这样同一个分类器即可用于 give / say / tp / effect / setblock / summon 等所有指令。
 */
export function classifyResponse(response) {
  const r = (response || "").trim();
  if (!r) return "valid"; // 空响应（如 /say）：语法已通过解析，仅无回显

  // 1. Brigadier 解析错误的位置标记 —— 跨指令最可靠的"语法非法"信号
  if (/<--\[HERE\]/.test(r)) return "invalid";

  // 2. 少数不带 HERE 标记的解析失败（措辞稳定，且不会出现在合法运行期回显中）
  if (/Unknown command|Unknown or incomplete command|Incorrect argument for command|^Expected |^Malformed /i.test(r)) {
    return "invalid";
  }

  // 3. 其余均视为语法合法（成功执行 / 运行期目标缺失 / 模式无操作）
  return "valid";
}
