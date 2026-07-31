/**
 * Snapshot tests for the multi-command builders + intent dispatcher.
 * Run: npx tsx src/logic/commands.test.mjs
 *
 * 语法真值来自 mc-verifier 的服务器实证（见各 builder 文件头的真值表）。
 */

import { buildSayCommand } from "./commands/say.ts";
import { buildEffectGiveCommand, buildEffectClearCommand } from "./commands/effect.ts";
import { buildTpCommand } from "./commands/tp.ts";
import { buildSetblockCommand } from "./commands/setblock.ts";
import { buildSummonCommand } from "./commands/summon.ts";
import { buildFillCommand } from "./commands/fill.ts";
import { buildCloneCommand } from "./commands/clone.ts";
import { buildEnchantCommand } from "./commands/enchant.ts";
import { buildExecuteCommand, sub } from "./commands/execute.ts";
import { buildScoreboardCommand } from "./commands/scoreboard.ts";
import { buildAttributeCommand } from "./commands/attribute.ts";
import { buildParticleCommand } from "./commands/particle.ts";
import { dispatchIntent, dispatchIntents } from "./dispatch.ts";
import { buildSystemPrompt, parseAiContent } from "./ai/prompt.ts";

let passed = 0;
let failed = 0;

function expect(label, actual, expected) {
  if (actual === expected) {
    console.log(`  PASS  ${label}`);
    passed++;
  } else {
    console.error(`  FAIL  ${label}`);
    console.error(`        expected: ${expected}`);
    console.error(`        actual:   ${actual}`);
    failed++;
  }
}

function expectThrows(label, fn) {
  try {
    fn();
    console.error(`  FAIL  ${label}`);
    console.error(`        expected a thrown error, got none`);
    failed++;
  } catch {
    console.log(`  PASS  ${label}`);
    passed++;
  }
}

// 现代族 / 旧族各取一个代表版本，覆盖两条 NBT 版本边界。
const MODERN = "java_1_21_5";
const MID = "java_1_21_4";

// ---------------- say ----------------
console.log("\n[say]");
expect("basic", buildSayCommand({ message: "hello world" }), "say hello world");
expect("withSlash", buildSayCommand({ message: "hi", withSlash: true }), "/say hi");

// ---------------- effect ----------------
console.log("\n[effect]");
expect(
  "give 仅效果",
  buildEffectGiveCommand({ target: "@a", effect: "speed" }),
  "effect give @a minecraft:speed",
);
expect(
  "give +时长",
  buildEffectGiveCommand({ target: "@a", effect: "speed", duration: 30 }),
  "effect give @a minecraft:speed 30",
);
expect(
  "give +等级（时长补默认 30）",
  buildEffectGiveCommand({ target: "@a", effect: "speed", amplifier: 2 }),
  "effect give @a minecraft:speed 30 2",
);
expect(
  "give +隐藏粒子（前置位置参数全部补出）",
  buildEffectGiveCommand({ target: "@a", effect: "speed", hideParticles: true }),
  "effect give @a minecraft:speed 30 0 true",
);
expect(
  "give infinite",
  buildEffectGiveCommand({ target: "@a", effect: "speed", duration: "infinite", amplifier: 1 }),
  "effect give @a minecraft:speed infinite 1",
);
expect("clear 全部", buildEffectClearCommand({ target: "@a" }), "effect clear @a");
expect(
  "clear 指定",
  buildEffectClearCommand({ target: "@a", effect: "minecraft:speed" }),
  "effect clear @a minecraft:speed",
);

// ---------------- tp ----------------
console.log("\n[tp]");
expect("绝对坐标", buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0" }), "tp @s 0 64 0");
expect("相对坐标", buildTpCommand({ targets: "@s", x: "~", y: "~", z: "~" }), "tp @s ~ ~ ~");
expect("本地坐标", buildTpCommand({ targets: "@s", x: "^", y: "^", z: "^1" }), "tp @s ^ ^ ^1");
expect(
  "旋转角",
  buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", yRot: "90", xRot: "45" }),
  "tp @s 0 64 0 90 45",
);
expect(
  "facing 优先于旋转角",
  buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", yRot: "90", xRot: "45", facingX: "10", facingY: "70", facingZ: "10" }),
  "tp @s 0 64 0 facing 10 70 10",
);
expect(
  "传送到实体",
  buildTpCommand({ targets: "@s", destination: "@e[type=pig,limit=1]" }),
  "tp @s @e[type=pig,limit=1]",
);
expect(
  "teleport 别名",
  buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", useTeleportAlias: true }),
  "teleport @s 0 64 0",
);

// ---------------- setblock ----------------
console.log("\n[setblock]");
expect(
  "基础",
  buildSetblockCommand({ version: MODERN, x: "~", y: "~", z: "~", block: "stone" }),
  "setblock ~ ~ ~ minecraft:stone",
);
expect(
  "方块状态",
  buildSetblockCommand({ version: MODERN, x: "~", y: "~", z: "~", block: "oak_log", blockstate: "axis=x" }),
  "setblock ~ ~ ~ minecraft:oak_log[axis=x]",
);
expect(
  "keep 模式",
  buildSetblockCommand({ version: MODERN, x: "~", y: "~", z: "~", block: "stone", mode: "keep" }),
  "setblock ~ ~ ~ minecraft:stone keep",
);
expect(
  "replace 是默认值，不输出",
  buildSetblockCommand({ version: MODERN, x: "~", y: "~", z: "~", block: "stone", mode: "replace" }),
  "setblock ~ ~ ~ minecraft:stone",
);
expect(
  "命令方块（内部命令去掉前导斜杠）",
  buildSetblockCommand({
    version: MODERN,
    x: "~", y: "~", z: "~",
    block: "command_block",
    blockstate: "facing=up",
    commandBlock: { command: "/say hi", auto: true },
  }),
  'setblock ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi",auto:1b}',
);
expect(
  "容器物品（Slot 大写 / count 小写）",
  buildSetblockCommand({
    version: MODERN,
    x: "~", y: "~", z: "~",
    block: "chest",
    containerItems: [{ slot: 0, item: { id: "diamond", count: 5 } }],
  }),
  'setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:5}]}',
);
expect(
  "容器物品带组件",
  buildSetblockCommand({
    version: MODERN,
    x: "~", y: "~", z: "~",
    block: "chest",
    containerItems: [{ slot: 1, item: { id: "stone", count: 1, components: { custom_name: `'{"text":"x"}'` } } }],
  }),
  `setblock ~ ~ ~ minecraft:chest{Items:[{Slot:1b,id:"minecraft:stone",count:1,components:{"minecraft:custom_name":'{"text":"x"}'}}]}`,
);

// ---------------- summon ----------------
console.log("\n[summon]");
expect("基础（无坐标无 NBT）", buildSummonCommand({ version: MODERN, entityType: "pig" }), "summon minecraft:pig");
expect(
  "指定坐标",
  buildSummonCommand({ version: MODERN, entityType: "pig", x: "0", y: "64", z: "0" }),
  "summon minecraft:pig 0 64 0",
);
expect(
  "有 NBT 时自动补出坐标",
  buildSummonCommand({ version: MODERN, entityType: "zombie", noAI: true, silent: true }),
  "summon minecraft:zombie ~ ~ ~ {NoAI:1b,Silent:1b}",
);
expect(
  "CustomName 走 SNBT 字符串",
  buildSummonCommand({ version: MODERN, entityType: "zombie", customName: [{ text: "Boss", color: "red" }] }),
  `summon minecraft:zombie ~ ~ ~ {CustomName:'[{"text":"Boss","color":"red"}]'}`,
);
expect(
  "Tags",
  buildSummonCommand({ version: MODERN, entityType: "pig", tags: ["a", "b"] }),
  'summon minecraft:pig ~ ~ ~ {Tags:["a","b"]}',
);
expect(
  "属性 1.21.5+（attributes/id/base，无 generic.）",
  buildSummonCommand({ version: MODERN, entityType: "zombie", attributes: [{ id: "max_health", base: 40 }] }),
  'summon minecraft:zombie ~ ~ ~ {attributes:[{id:"minecraft:max_health",base:40d}]}',
);
expect(
  "属性 1.21.4（Attributes/Name/Base，带 generic.）",
  buildSummonCommand({ version: MID, entityType: "zombie", attributes: [{ id: "max_health", base: 40 }] }),
  'summon minecraft:zombie ~ ~ ~ {Attributes:[{Name:"minecraft:generic.max_health",Base:40d}]}',
);
expect(
  "状态效果（两版本同格式）",
  buildSummonCommand({ version: MID, entityType: "zombie", effects: [{ id: "speed", duration: 200, amplifier: 1 }] }),
  'summon minecraft:zombie ~ ~ ~ {active_effects:[{id:"minecraft:speed",duration:200,amplifier:1b,show_particles:1b}]}',
);
expect(
  "装备 1.21.5+（equipment compound）",
  buildSummonCommand({ version: MODERN, entityType: "zombie", equipment: { mainhand: { id: "diamond_sword" } } }),
  'summon minecraft:zombie ~ ~ ~ {equipment:{mainhand:{id:"minecraft:diamond_sword",count:1}}}',
);
expect(
  "装备 1.21.4（HandItems，空槽占位，无护甲则不输出 ArmorItems）",
  buildSummonCommand({ version: MID, entityType: "zombie", equipment: { mainhand: { id: "diamond_sword" } } }),
  'summon minecraft:zombie ~ ~ ~ {HandItems:[{id:"minecraft:diamond_sword",count:1},{}]}',
);
expect(
  "装备 1.21.4 护甲（ArmorItems 定长 feet,legs,chest,head）",
  buildSummonCommand({ version: MID, entityType: "zombie", equipment: { head: { id: "diamond_helmet" } } }),
  'summon minecraft:zombie ~ ~ ~ {ArmorItems:[{},{},{},{id:"minecraft:diamond_helmet",count:1}]}',
);
expect(
  "Passengers",
  buildSummonCommand({ version: MODERN, entityType: "pig", passengers: [{ entityType: "chicken" }] }),
  'summon minecraft:pig ~ ~ ~ {Passengers:[{id:"minecraft:chicken"}]}',
);
expect(
  "血量：health 单独生效（Health 浮点数）",
  buildSummonCommand({ version: MODERN, entityType: "zombie", health: 40 }),
  "summon minecraft:zombie ~ ~ ~ {Health:40f}",
);
expect(
  "血量：attributes(max_health) + health 搭配使用，同时改上限和当前值",
  buildSummonCommand({ version: MODERN, entityType: "zombie", attributes: [{ id: "max_health", base: 40 }], health: 40 }),
  'summon minecraft:zombie ~ ~ ~ {Health:40f,attributes:[{id:"minecraft:max_health",base:40d}]}',
);
expect(
  "朝向：Rotation:[yaw,pitch]",
  buildSummonCommand({ version: MODERN, entityType: "zombie", rotation: [90, 0] }),
  "summon minecraft:zombie ~ ~ ~ {Rotation:[90f,0f]}",
);
expect(
  "装备附魔 1.21.5+（enchantments 组件，无 levels 包装）",
  buildSummonCommand({
    version: MODERN,
    entityType: "zombie",
    equipment: { mainhand: { id: "diamond_sword", enchantments: [{ id: "sharpness", level: 5 }] } },
  }),
  'summon minecraft:zombie ~ ~ ~ {equipment:{mainhand:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{sharpness:5}}}}}',
);
expect(
  "装备附魔 java_1_21（levels 包装）",
  buildSummonCommand({
    version: "java_1_21",
    entityType: "zombie",
    equipment: { mainhand: { id: "diamond_sword", enchantments: [{ id: "sharpness", level: 5 }] } },
  }),
  'summon minecraft:zombie ~ ~ ~ {HandItems:[{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{levels:{sharpness:5}}}},{}]}',
);

// ---------------- fill ----------------
console.log("\n[fill]");
expect(
  "基础",
  buildFillCommand({ from: ["0", "64", "0"], to: ["10", "70", "10"], block: "stone" }),
  "fill 0 64 0 10 70 10 minecraft:stone",
);
expect(
  "hollow 模式",
  buildFillCommand({ from: ["~", "~", "~"], to: ["~5", "~5", "~5"], block: "glass", mode: "hollow" }),
  "fill ~ ~ ~ ~5 ~5 ~5 minecraft:glass hollow",
);
expect(
  "replace 过滤方块",
  buildFillCommand({
    from: ["0", "64", "0"], to: ["10", "70", "10"],
    block: "air",
    replaceFilter: { block: "water" },
  }),
  "fill 0 64 0 10 70 10 minecraft:air replace minecraft:water",
);
expect(
  "方块状态 + NBT 紧贴方块 id",
  buildFillCommand({
    from: ["~", "~", "~"], to: ["~", "~", "~"],
    block: "command_block", blockstate: "facing=up", nbt: '{Command:"say hi"}',
  }),
  'fill ~ ~ ~ ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi"}',
);

// ---------------- clone ----------------
console.log("\n[clone]");
expect(
  "基础",
  buildCloneCommand({ begin: ["0", "64", "0"], end: ["10", "70", "10"], destination: ["20", "64", "20"] }),
  "clone 0 64 0 10 70 10 20 64 20",
);
expect(
  "跨维度",
  buildCloneCommand({
    fromDimension: "the_nether",
    begin: ["0", "64", "0"], end: ["10", "70", "10"],
    toDimension: "overworld",
    destination: ["20", "64", "20"],
  }),
  "clone from minecraft:the_nether 0 64 0 10 70 10 to minecraft:overworld 20 64 20",
);
expect(
  "filtered 带过滤方块",
  buildCloneCommand({
    begin: ["0", "64", "0"], end: ["10", "70", "10"], destination: ["20", "64", "20"],
    maskMode: "filtered", filter: { block: "stone" },
  }),
  "clone 0 64 0 10 70 10 20 64 20 filtered minecraft:stone",
);
expect(
  "move 模式自动补 replace",
  buildCloneCommand({
    begin: ["0", "64", "0"], end: ["10", "70", "10"], destination: ["20", "64", "20"],
    cloneMode: "move",
  }),
  "clone 0 64 0 10 70 10 20 64 20 replace move",
);
expectThrows("filtered 缺 filter 应报错", () =>
  buildCloneCommand({
    begin: ["0", "64", "0"], end: ["10", "70", "10"], destination: ["20", "64", "20"],
    maskMode: "filtered",
  }),
);

// ---------------- enchant ----------------
console.log("\n[enchant]");
expect("无等级", buildEnchantCommand({ targets: "@s", enchantment: "sharpness" }), "enchant @s minecraft:sharpness");
expect(
  "带等级",
  buildEnchantCommand({ targets: "@s", enchantment: "minecraft:unbreaking", level: 3 }),
  "enchant @s minecraft:unbreaking 3",
);

// ---------------- execute ----------------
console.log("\n[execute]");
expect(
  "as + at + run",
  buildExecuteCommand({ subcommands: [sub.as("@a"), sub.at("@s")], run: "say hi" }),
  "execute as @a at @s run say hi",
);
expect(
  "run 的前导斜杠被去掉",
  buildExecuteCommand({ subcommands: [sub.at("@s")], run: "/summon minecraft:tnt ~ ~ ~" }),
  "execute at @s run summon minecraft:tnt ~ ~ ~",
);
expect(
  "纯条件测试（无 run，以 if 结尾）",
  buildExecuteCommand({ subcommands: [sub.ifEntity("@e[type=pig]")] }),
  "execute if entity @e[type=pig]",
);
expect(
  "箭矢触地检测（AI 组合技的核心形态）",
  buildExecuteCommand({ subcommands: [sub.at("@e[type=arrow,nbt={inGround:1b}]")], run: "summon tnt ~ ~ ~" }),
  "execute at @e[type=arrow,nbt={inGround:1b}] run summon tnt ~ ~ ~",
);
expectThrows("无子命令应报错", () => buildExecuteCommand({ subcommands: [] }));
expectThrows("无 run 且不以 if/unless 结尾应报错", () =>
  buildExecuteCommand({ subcommands: [sub.as("@a")] }),
);

// ---------------- scoreboard ----------------
console.log("\n[scoreboard]");
expect(
  "objectives add",
  buildScoreboardCommand({ action: { kind: "objectives_add", objective: "kills", criteria: "playerKillCount" } }),
  "scoreboard objectives add kills playerKillCount",
);
expect(
  "objectives add 带显示名（文本组件 JSON）",
  buildScoreboardCommand({
    action: { kind: "objectives_add", objective: "kills", criteria: "dummy", displayName: "击杀数" },
  }),
  'scoreboard objectives add kills dummy {"text":"击杀数"}',
);
expect(
  "objectives setdisplay",
  buildScoreboardCommand({ action: { kind: "objectives_setdisplay", slot: "sidebar", objective: "kills" } }),
  "scoreboard objectives setdisplay sidebar kills",
);
expect(
  "players set",
  buildScoreboardCommand({ action: { kind: "players_set", targets: "@a", objective: "kills", score: 0 } }),
  "scoreboard players set @a kills 0",
);
expect(
  "players operation",
  buildScoreboardCommand({
    action: {
      kind: "players_operation",
      targets: "@s", objective: "a", operation: "+=", source: "@s", sourceObjective: "b",
    },
  }),
  "scoreboard players operation @s a += @s b",
);
expect(
  "players reset 全部",
  buildScoreboardCommand({ action: { kind: "players_reset", targets: "@a" } }),
  "scoreboard players reset @a",
);

// ---------------- attribute ----------------
console.log("\n[attribute]");
expect(
  "base set 1.21.5+（无 generic. 前缀）",
  buildAttributeCommand({ version: MODERN, target: "@s", attribute: "max_health", action: { kind: "base_set", value: 40 } }),
  "attribute @s minecraft:max_health base set 40",
);
expect(
  "base set 1.21.4（带 generic. 前缀）",
  buildAttributeCommand({ version: MID, target: "@s", attribute: "max_health", action: { kind: "base_set", value: 40 } }),
  "attribute @s minecraft:generic.max_health base set 40",
);
expect(
  "输入已带前缀时按版本归一化",
  buildAttributeCommand({
    version: MODERN, target: "@s", attribute: "minecraft:generic.max_health", action: { kind: "base_set", value: 40 },
  }),
  "attribute @s minecraft:max_health base set 40",
);
expect(
  "modifier add 1.21+（id + value + 新运算名）",
  buildAttributeCommand({
    version: MODERN, target: "@s", attribute: "max_health",
    action: { kind: "modifier_add", id: "my_buff", value: 4, operation: "add" },
  }),
  "attribute @s minecraft:max_health modifier add minecraft:my_buff 4 add_value",
);
expect(
  "modifier add 1.20.5（uuid + name + 旧运算名）",
  buildAttributeCommand({
    version: "java_1_20_5", target: "@s", attribute: "max_health",
    action: { kind: "modifier_add", id: "uuid-1", name: "buff", value: 4, operation: "multiply_total" },
  }),
  "attribute @s minecraft:generic.max_health modifier add uuid-1 buff 4 multiply",
);
expect(
  "get 带缩放",
  buildAttributeCommand({ version: MODERN, target: "@s", attribute: "max_health", action: { kind: "get", scale: 0.5 } }),
  "attribute @s minecraft:max_health get 0.5",
);

// ---------------- particle ----------------
console.log("\n[particle]");
expect("最简单形式（无坐标无扩展参数）", buildParticleCommand({ name: "flame" }), "particle minecraft:flame");
expect(
  "带位置和扩展参数",
  buildParticleCommand({ name: "flame", x: "~", y: "~1", z: "~", dx: 0.3, dy: 0.3, dz: 0.3, speed: 0.02, count: 20 }),
  "particle minecraft:flame ~ ~1 ~ 0.3 0.3 0.3 0.02 20",
);
expect(
  "force 模式",
  buildParticleCommand({
    name: "totem_of_undying", x: "~", y: "~1", z: "~",
    dx: 0.5, dy: 0.5, dz: 0.5, speed: 0, count: 100, mode: "force",
  }),
  "particle minecraft:totem_of_undying ~ ~1 ~ 0.5 0.5 0.5 0 100 force",
);
expect(
  "viewers 指定观众（自动补 normal）",
  buildParticleCommand({ name: "flame", x: "~", y: "~", z: "~", count: 5, viewers: "@a" }),
  "particle minecraft:flame ~ ~ ~ 0 0 0 1 5 normal @a",
);
expect(
  "参数化粒子（dust 带颜色）：只给花括号前的部分加前缀",
  buildParticleCommand({ name: "dust{color:[1.0,0.2,0.2],scale:1.5}", x: "~", y: "~", z: "~", count: 10 }),
  'particle minecraft:dust{color:[1.0,0.2,0.2],scale:1.5} ~ ~ ~ 0 0 0 1 10',
);
expect(
  "withSlash",
  buildParticleCommand({ name: "flame", withSlash: true }),
  "/particle minecraft:flame",
);

// ---------------- dispatch ----------------
console.log("\n[dispatch]");
{
  const r = dispatchIntent({ command: "say", form: { message: "hi" } }, MODERN);
  expect("say 意图分派成功", r.command, "say hi");
  expect("成功时 error 为 null", r.error, null);
}
{
  // give 走本仓库既有的 buildGiveCommand + normalizeForm（脏数据自动补全）
  const r = dispatchIntent({ command: "give", form: { item: "minecraft:mace", target: "@s", count: 1 } }, MODERN);
  expect("give 意图复用现有 give builder", r.command, "give @s minecraft:mace 1");
}
{
  // version 由分派器注入，意图里不需要写
  const r = dispatchIntent({ command: "summon", form: { entityType: "zombie", attributes: [{ id: "max_health", base: 40 }] } }, MID);
  expect(
    "版本由分派器注入（旧属性格式）",
    r.command,
    'summon minecraft:zombie ~ ~ ~ {Attributes:[{Name:"minecraft:generic.max_health",Base:40d}]}',
  );
}
{
  // 非法意图不应抛出，而是被捕获成 error
  const r = dispatchIntent({ command: "execute", form: { subcommands: [] } }, MODERN);
  expect("非法意图 command 为 null", r.command, null);
  expect("非法意图带错误信息", typeof r.error === "string" && r.error.length > 0, true);
}
{
  const results = dispatchIntents(
    [
      { command: "say", form: { message: "a" } },
      { command: "execute", form: { subcommands: [] } },
      { command: "enchant", form: { targets: "@s", enchantment: "sharpness" } },
    ],
    MODERN,
  );
  expect("批量分派保持顺序与长度", results.length, 3);
  expect("批量分派中单条失败不影响其他", results[2].command, "enchant @s minecraft:sharpness");
}
{
  const r = dispatchIntent({ command: "particle", form: { name: "flame", count: 5 } }, MODERN);
  expect("particle 意图分派成功", r.command, "particle minecraft:flame ~ ~ ~ 0 0 0 1 5");
}
{
  // 未知 command 的报错信息应该是可读的具体值，不是整个意图对象的 JSON dump
  const r = dispatchIntent({ command: "no_such_thing", form: {} }, MODERN);
  expect("未知指令类型报错信息可读", r.error, '未知指令类型: "no_such_thing"');
}

// ---------------- AI prompt / 解析 ----------------
console.log("\n[ai prompt]");
{
  const prompt = buildSystemPrompt(MODERN);
  // 这两条是实测真值，写错会让 AI 生成永远匹配不到的选择器（见 mc-verifier K 组探针）
  expect("提示词教了箭用 inGround", prompt.includes("inGround:1b"), true);
  expect("提示词教了掉落物用 OnGround", prompt.includes("OnGround:1b"), true);
  expect("提示词点明两者不可混用", prompt.includes("用 OnGround 过滤箭会永远匹配不到"), true);
  expect("提示词注入了附魔 id 表", prompt.includes("minecraft:sharpness"), true);
  expect("提示词注入了药水效果 id 表", prompt.includes("minecraft:jump_boost"), true);
  expect("提示词声明了目标版本", prompt.includes(MODERN), true);
  // 血量应该用 attribute，不要用 effect 凑——这是本轮用户反馈修的核心问题之一
  expect("提示词教了血量要用 attribute + health 而不是 effect", prompt.includes("绝不要用 effect_give"), true);
  expect("提示词提到 health 字段要和 attributes 搭配", prompt.includes('health: 40'), true);
  // effect 无限时长
  expect("提示词教了 effect 无限用 infinite 而不是塞极大整数", prompt.includes('"infinite"'), true);
  // particle
  expect("提示词包含 particle 用法", prompt.includes("particle minecraft:flame"), true);
  // 朝向
  expect("提示词教了 summon rotation 朝向", prompt.includes("rotation: [yaw, pitch]"), true);
  expect("提示词教了 execute facing/rotated 用法", prompt.includes("facing entity"), true);
  // 判定/锚点实体
  expect("提示词教了 marker 判定实体", prompt.includes("minecraft:marker"), true);
  expect("提示词教了盔甲架 Marker 标签", prompt.includes("Marker:1b"), true);
  // 特殊计分板判据
  expect("提示词教了 used 统计判据", prompt.includes("minecraft.used:minecraft"), true);
  expect("提示词教了 sneak_time 统计判据", prompt.includes("minecraft.custom:minecraft.sneak_time"), true);
  expect("提示词给了拔刀剑组合范例", prompt.includes("拔刀剑"), true);
  // 装备附魔要走结构化字段，不要求 AI 手拼 SNBT
  expect("提示词教了装备附魔走结构化 enchantments 字段", prompt.includes("equipment.<slot>.enchantments"), true);
}
{
  const r = parseAiContent('{"intents":[{"command":"say","form":{"message":"hi"}}],"explanation":"打个招呼"}');
  expect("解析 intents", r.intents.length, 1);
  expect("解析 explanation", r.explanation, "打个招呼");
}
{
  // 模型有时会把 JSON 包在代码块里
  const r = parseAiContent('```json\n{"intents":[],"explanation":"x"}\n```');
  expect("容忍 ```json 代码块包裹", r.explanation, "x");
}
{
  // 复现用户反馈的「未知指令类型: explanation」bug：模型偶尔把 explanation
  // 错放进 intents 数组里（没有 command 字段）。应静默兜底捞出来当 explanation，
  // 而不是当成一条非法指令传给 dispatch 报错。
  const r = parseAiContent(
    '{"intents":[{"command":"say","form":{"message":"hi"}},{"explanation":"这是解释"}],"explanation":""}',
  );
  expect("explanation 误入 intents 数组时会被过滤掉，不进入 intents", r.intents.length, 1);
  expect("过滤掉的 explanation 被兜底捞出来用", r.explanation, "这是解释");
}
{
  // 顶层 explanation 优先于误入数组里的那个
  const r = parseAiContent(
    '{"intents":[{"explanation":"数组里的"}],"explanation":"顶层的"}',
  );
  expect("顶层 explanation 优先", r.explanation, "顶层的");
  expect("误入数组的项被丢弃", r.intents.length, 0);
}
expectThrows("非 JSON 应报错", () => parseAiContent("对不起，我做不到"));
expectThrows("缺 intents 数组应报错", () => parseAiContent('{"explanation":"x"}'));

// ---------------- 端到端：AI 响应 → 命令字符串 ----------------
console.log("\n[端到端：爆炸箭 / 地雷]");
{
  // 模拟 AI 按提示词产出的「爆炸箭」意图，验证整条链路落地成实测可用的命令
  const ai = parseAiContent(
    JSON.stringify({
      intents: [
        { command: "give", form: { item: "minecraft:bow", count: 1, enchantments: [{ id: "minecraft:power", level: 5 }] } },
        {
          command: "execute",
          form: {
            subcommands: ["at @e[type=minecraft:arrow,nbt={inGround:1b}]"],
            run: "summon minecraft:tnt ~ ~ ~ {fuse:0s}",
          },
        },
      ],
      explanation: "把第二条放进循环命令方块",
    }),
  );
  const [giveCmd, executeCmd] = dispatchIntents(ai.intents, MODERN).map((r) => r.command);
  // 1.21.5+ 的附魔是扁平写法，levels:{} 外层只属于 1.21/1.21.1（见 probes.mjs ench_flat / ench_levels）
  expect(
    "AI 意图 → 附power的弓",
    giveCmd,
    "give @a minecraft:bow[enchantments={power:5}] 1",
  );
  expect(
    "AI 意图 → 箭落地生成 TNT（选择器与实测真值一致）",
    executeCmd,
    "execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",
  );
}
{
  // 地雷：掉落物落地即引爆，并把触发过的掉落物清掉（否则每 tick 重复触发）
  const r = dispatchIntent(
    {
      command: "setblock",
      form: {
        x: "~", y: "~", z: "~",
        block: "repeating_command_block",
        commandBlock: {
          command: "execute at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:\"minecraft:tnt\"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",
          auto: true,
        },
      },
    },
    MODERN,
  );
  expect(
    "地雷命令方块（嵌套引号正确转义，与实测读回一致）",
    r.command,
    'setblock ~ ~ ~ minecraft:repeating_command_block{Command:"execute at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:\\"minecraft:tnt\\"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",auto:1b}',
  );
}
{
  // 复现用户报告的场景：召唤一只 40 血、拿附魔钻石剑、不会动的僵尸。
  // 验证 health + attributes 搭配、equipment.enchantments 结构化字段的完整链路。
  const ai = parseAiContent(
    JSON.stringify({
      intents: [
        {
          command: "summon",
          form: {
            entityType: "minecraft:zombie",
            noAI: true,
            health: 40,
            attributes: [{ id: "max_health", base: 40 }],
            equipment: {
              mainhand: { id: "minecraft:diamond_sword", enchantments: [{ id: "minecraft:sharpness", level: 5 }] },
            },
          },
        },
      ],
      explanation: "40 血用属性+当前值同时设置，剑的附魔走结构化字段",
    }),
  );
  const [summonCmd] = dispatchIntents(ai.intents, MODERN).map((r) => r.command);
  expect(
    "端到端：40 血不会动的僵尸 + 附魔剑",
    summonCmd,
    'summon minecraft:zombie ~ ~ ~ {NoAI:1b,Health:40f,attributes:[{id:"minecraft:max_health",base:40d}],equipment:{mainhand:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantments":{sharpness:5}}}}}',
  );
}

// --- summary ---
console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
