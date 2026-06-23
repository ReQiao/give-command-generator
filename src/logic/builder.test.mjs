/**
 * Lightweight snapshot tests for builder.ts (Node, no test framework needed).
 * Run: node src/logic/builder.test.mjs
 */

import { createDefaultForm, buildGiveCommand } from "./builder.ts";

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

// --- helpers ---
function base(version) {
  const f = createDefaultForm();
  f.version = version;
  f.item = "minecraft:stone";
  f.target = "@a";
  f.count = 1;
  return f;
}

// --- 1. Java 1.21.11+ basic ---
{
  const f = base("java_1_21_11_plus");
  expect(
    "1.21.11+ basic item",
    buildGiveCommand(f),
    "give @a minecraft:stone 1",
  );
}

// --- 2. Java 1.21.11+ item_name ---
{
  const f = base("java_1_21_11_plus");
  f.itemName = [[{ text: "My Item" }]];
  const cmd = buildGiveCommand(f);
  expect(
    "1.21.11+ item_name present",
    cmd.includes("item_name="),
    true,
  );
}

// --- 3. Java 1.21 item_name (was broken, now fixed) ---
{
  const f = base("java_1_21");
  f.itemName = [[{ text: "物品名称", underlined: true, color: "#000599" }]];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 item_name present",
    cmd.includes("item_name="),
    true,
  );
  expect(
    "java_1_21 item_name is SNBT single-quoted string",
    cmd.includes("item_name='"),
    true,
  );
}

// --- 4. Java 1.21.1 item_name ---
{
  const f = base("java_1_21_1");
  f.itemName = [[{ text: "名称" }]];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21_1 item_name present",
    cmd.includes("item_name='"),
    true,
  );
}

// --- 5. Java 1.21 custom_name SNBT format ---
{
  const f = base("java_1_21");
  f.displayName = [[{ text: "字", bold: true, italic: false, color: "#000599" }]];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 custom_name uses single-quoted SNBT",
    cmd.includes("custom_name='"),
    true,
  );
}

// --- 6. Java 1.21 enchantments with levels wrapper ---
{
  const f = base("java_1_21");
  f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 enchantments has levels wrapper",
    cmd.includes("enchantments={levels:{unbreaking:3}}"),
    true,
  );
}

// --- 7. Java 1.21 attribute type uses generic. prefix ---
{
  const f = base("java_1_21");
  f.attributes = [{ type: "armor", amount: 10, slot: "任意", operation: "加算", id: "99" }];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 attribute type has generic. prefix",
    cmd.includes('"generic.armor"'),
    true,
  );
  expect(
    "java_1_21 attribute_modifiers has modifiers wrapper",
    cmd.includes("attribute_modifiers={modifiers:["),
    true,
  );
}

// --- 8. Java 1.21.11+ enchantments (no levels wrapper) ---
{
  const f = base("java_1_21_11_plus");
  f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
  const cmd = buildGiveCommand(f);
  expect(
    "1.21.11+ enchantments no levels wrapper",
    cmd.includes("enchantments={unbreaking:3}"),
    true,
  );
}

// --- 9. Java 1.21 food with eat_seconds merging ---
{
  const f = base("java_1_21");
  f.foodEnabled = true;
  f.nutrition = 5;
  f.saturation = 6;
  f.consumableEnabled = true;
  f.consumeSeconds = 2;
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 food contains eat_seconds",
    cmd.includes("eat_seconds:2"),
    true,
  );
  expect(
    "java_1_21 no independent consumable",
    !cmd.includes("consumable="),
    true,
  );
}

// --- 10. Bedrock not polluted by Java changes ---
{
  const f = base("bedrock");
  const cmd = buildGiveCommand(f);
  expect(
    "bedrock basic format",
    cmd.startsWith("give @a stone 1 0"),
    true,
  );
}

// --- 11. Java 1.21 lore is SNBT string array ---
{
  const f = base("java_1_21");
  f.lore = [[{ text: "第一行" }], [{ text: "第二行" }]];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 lore uses single-quoted SNBT entries",
    cmd.includes("lore=['") || cmd.includes("lore=[\""),
    true,
  );
  // Each lore line should be a quoted SNBT string, not a direct JSON array
  expect(
    "java_1_21 lore is array of quoted strings",
    (cmd.match(/lore=\['/g) || []).length > 0 || cmd.includes("lore=['"),
    true,
  );
}

// --- 12. Java 1.21 unbreakable ---
{
  const f = base("java_1_21");
  f.unbreakable = true;
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 unbreakable={}",
    cmd.includes("unbreakable={}"),
    true,
  );
}

// --- 13. Java 1.21 no glider, no death_protection ---
{
  const f = base("java_1_21");
  f.glider = true; // should be ignored by builder (but UI prunes it)
  f.deathProtection = true;
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 builder does not output glider",
    !cmd.includes("glider"),
    true,
  );
  expect(
    "java_1_21 builder does not output death_protection",
    !cmd.includes("death_protection"),
    true,
  );
}

// --- 14. Java 1.21 can_place_on / can_break use predicates wrapper (server-verified) ---
{
  const f = base("java_1_21");
  f.blockLimits = [
    { block: "minecraft:stone", type: "place" },
    { block: "minecraft:dirt", type: "break" },
  ];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 can_place_on uses predicates wrapper",
    cmd.includes('can_place_on={predicates:[{blocks:"minecraft:stone"}]}'),
    true,
  );
  expect(
    "java_1_21 can_break uses predicates wrapper",
    cmd.includes('can_break={predicates:[{blocks:"minecraft:dirt"}]}'),
    true,
  );
}

// --- 15. Java 1.21.11+ can_place_on / can_break use direct quoted list (server-verified) ---
{
  const f = base("java_1_21_11_plus");
  f.blockLimits = [
    { block: "minecraft:stone", type: "place" },
    { block: "minecraft:dirt", type: "break" },
  ];
  const cmd = buildGiveCommand(f);
  expect(
    "1.21.11+ can_place_on uses direct quoted list",
    cmd.includes('can_place_on=[{blocks:"minecraft:stone"}]'),
    true,
  );
  expect(
    "1.21.11+ can_break uses direct quoted list",
    cmd.includes('can_break=[{blocks:"minecraft:dirt"}]'),
    true,
  );
  expect(
    "1.21.11+ can_place_on has no predicates wrapper",
    !cmd.includes("predicates"),
    true,
  );
}

// --- 16. Java 1.21 attribute id always quoted, even numeric input (server-verified) ---
{
  const f = base("java_1_21");
  f.attributes = [{ type: "armor", amount: 1, slot: "any", operation: "add_value", id: "123" }];
  const cmd = buildGiveCommand(f);
  expect(
    "java_1_21 numeric attribute id is quoted",
    cmd.includes('id:"123"'),
    true,
  );
}

// --- 17. Java 1.21.11+ tooltip_display hidden_components are quoted (server-verified) ---
{
  const f = base("java_1_21_11_plus");
  f.hiddenComponents = "enchantments";
  const cmd = buildGiveCommand(f);
  expect(
    "1.21.11+ tooltip_display hidden_components quoted",
    cmd.includes('hidden_components:["minecraft:enchantments"]'),
    true,
  );
}

// --- 18. Java 1.21.2 text uses SNBT single-quoted strings (server-verified) ---
{
  const f = base("java_1_21_2");
  f.displayName = [[{ text: "hi" }]];
  f.itemName = [[{ text: "name" }]];
  f.lore = [[{ text: "a" }]];
  const cmd = buildGiveCommand(f);
  expect("1.21.2 custom_name SNBT string", cmd.includes("custom_name='"), true);
  expect("1.21.2 item_name SNBT string", cmd.includes("item_name='"), true);
  expect("1.21.2 lore SNBT string array", cmd.includes("lore=['"), true);
}

// --- 19. Java 1.21.2 enchantments flat, no levels wrapper (server-verified) ---
{
  const f = base("java_1_21_2");
  f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
  const cmd = buildGiveCommand(f);
  expect("1.21.2 enchantments flat", cmd.includes("enchantments={unbreaking:3}"), true);
  expect("1.21.2 enchantments no levels wrapper", !cmd.includes("levels"), true);
}

// --- 20. Java 1.21.2 attribute uses modern array, stripped unquoted type (server-verified) ---
{
  const f = base("java_1_21_2");
  f.attributes = [{ type: "armor", amount: 2, slot: "any", operation: "add_value", id: "x" }];
  const cmd = buildGiveCommand(f);
  expect("1.21.2 attribute_modifiers direct array", cmd.includes("attribute_modifiers=[{type:armor"), true);
  expect("1.21.2 attribute no modifiers wrapper", !cmd.includes("modifiers:["), true);
  expect("1.21.2 attribute id quoted", cmd.includes('id:"x"'), true);
}

// --- 21. Java 1.21.2 can_place_on / can_break use predicates wrapper (server-verified) ---
{
  const f = base("java_1_21_2");
  f.blockLimits = [
    { block: "minecraft:stone", type: "place" },
    { block: "minecraft:dirt", type: "break" },
  ];
  const cmd = buildGiveCommand(f);
  expect("1.21.2 can_place_on predicates wrapper", cmd.includes('can_place_on={predicates:[{blocks:"minecraft:stone"}]}'), true);
  expect("1.21.2 can_break predicates wrapper", cmd.includes('can_break={predicates:[{blocks:"minecraft:dirt"}]}'), true);
}

// --- 22. Java 1.21.2 supports glider / death_protection / consumable ---
{
  const f = base("java_1_21_2");
  f.glider = true;
  f.deathProtection = true;
  f.consumableEnabled = true;
  f.consumeSeconds = 2;
  const cmd = buildGiveCommand(f);
  expect("1.21.2 glider supported", cmd.includes("glider={}"), true);
  expect("1.21.2 death_protection supported", cmd.includes("death_protection"), true);
  expect("1.21.2 consumable separate", cmd.includes("consumable={consume_seconds:2"), true);
}

// --- 23. Java 1.21.2 omits tooltip_display (unsupported, server-verified) ---
{
  const f = base("java_1_21_2");
  f.hiddenComponents = "enchantments";
  const cmd = buildGiveCommand(f);
  expect("1.21.2 no tooltip_display", !cmd.includes("tooltip_display"), true);
}

// --- 24. Java 1.21.3 produces identical output to 1.21.2 ---
{
  const make = (version) => {
    const f = base(version);
    f.displayName = [[{ text: "hi" }]];
    f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
    f.glider = true;
    f.blockLimits = [{ block: "minecraft:stone", type: "place" }];
    return buildGiveCommand(f);
  };
  expect("1.21.3 same syntax as 1.21.2", make("java_1_21_3"), make("java_1_21_2"));
}

// --- 25. Java 1.21.4 produces identical output to 1.21.2 (mid family) ---
{
  const make = (version) => {
    const f = base(version);
    f.displayName = [[{ text: "hi" }]];
    f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
    f.glider = true;
    f.blockLimits = [{ block: "minecraft:stone", type: "place" }];
    return buildGiveCommand(f);
  };
  expect("1.21.4 same syntax as 1.21.2 (mid)", make("java_1_21_4"), make("java_1_21_2"));
}

// --- 26. Java 1.21.5 produces identical output to 1.21.11+ (modern) ---
{
  const make = (version) => {
    const f = base(version);
    f.displayName = [[{ text: "hi" }]];
    f.blockLimits = [{ block: "minecraft:stone", type: "place" }];
    f.hiddenComponents = "enchantments";
    return buildGiveCommand(f);
  };
  expect("1.21.5 same syntax as 1.21.11+ (modern)", make("java_1_21_5"), make("java_1_21_11_plus"));
}

// --- 27. Java 1.21.6 produces identical output to 1.21.11+ ---
{
  const f1 = base("java_1_21_6");
  const f2 = base("java_1_21_11_plus");
  f1.enchantments = f2.enchantments = [{ id: "minecraft:unbreaking", level: 2 }];
  expect("1.21.6 same syntax as 1.21.11+", buildGiveCommand(f1), buildGiveCommand(f2));
}

// --- 28. Java 1.21.9 produces identical output to 1.21.11+ ---
{
  const f1 = base("java_1_21_9");
  const f2 = base("java_1_21_11_plus");
  f1.deathProtection = f2.deathProtection = true;
  expect("1.21.9 same syntax as 1.21.11+", buildGiveCommand(f1), buildGiveCommand(f2));
}

// --- 29. Java 26.1 and 26.2+ produce identical output to 1.21.11+ ---
{
  const f1 = base("java_26_1");
  const f2 = base("java_26_2_plus");
  const f3 = base("java_1_21_11_plus");
  f1.glider = f2.glider = f3.glider = true;
  f1.hiddenComponents = f2.hiddenComponents = f3.hiddenComponents = "enchantments";
  expect("26.1 same syntax as 1.21.11+", buildGiveCommand(f1), buildGiveCommand(f3));
  expect("26.2+ same syntax as 1.21.11+", buildGiveCommand(f2), buildGiveCommand(f3));
}

// --- 30. Java 1.20.5 text uses SNBT single-quoted strings ---
{
  const f = base("java_1_20_5");
  f.displayName = [[{ text: "hi" }]];
  f.itemName = [[{ text: "name" }]];
  f.lore = [[{ text: "a" }]];
  const cmd = buildGiveCommand(f);
  expect("1.20.5 custom_name SNBT string", cmd.includes("custom_name='"), true);
  expect("1.20.5 item_name SNBT string", cmd.includes("item_name='"), true);
  expect("1.20.5 lore SNBT string array", cmd.includes("lore=['"), true);
}

// --- 31. Java 1.20.5 can_place_on uses predicates wrapper ---
{
  const f = base("java_1_20_5");
  f.blockLimits = [{ block: "minecraft:stone", type: "place" }];
  const cmd = buildGiveCommand(f);
  expect("1.20.5 can_place_on predicates wrapper", cmd.includes('can_place_on={predicates:[{blocks:"minecraft:stone"}]}'), true);
}

// --- 32. Java 1.20.5 does not output consumable, glider, death_protection ---
{
  const f = base("java_1_20_5");
  f.consumableEnabled = true;
  f.consumeSeconds = 2;
  f.glider = true;
  f.deathProtection = true;
  const cmd = buildGiveCommand(f);
  expect("1.20.5 no consumable", !cmd.includes("consumable="), true);
  expect("1.20.5 no glider", !cmd.includes("glider"), true);
  expect("1.20.5 no death_protection", !cmd.includes("death_protection"), true);
}

// --- 33. Java 1.20.5 does not output attribute_modifiers ---
{
  const f = base("java_1_20_5");
  f.attributes = [{ type: "armor", amount: 5, slot: "任意", operation: "加算", id: "test" }];
  const cmd = buildGiveCommand(f);
  expect("1.20.5 no attribute_modifiers", !cmd.includes("attribute_modifiers"), true);
}

// --- 34. Java 1.20.5 does not output tooltip_display ---
{
  const f = base("java_1_20_5");
  f.hiddenComponents = "enchantments";
  const cmd = buildGiveCommand(f);
  expect("1.20.5 no tooltip_display", !cmd.includes("tooltip_display"), true);
}

// --- 35. Java 1.20.5 enchantments flat format ---
{
  const f = base("java_1_20_5");
  f.enchantments = [{ id: "minecraft:unbreaking", level: 3 }];
  const cmd = buildGiveCommand(f);
  expect("1.20.5 enchantments flat", cmd.includes("enchantments={unbreaking:3}"), true);
}

// ====== setblock / summon builders ======

import { buildSetblockCommand } from "./commands/setblock.ts";
import { buildSummonCommand } from "./commands/summon.ts";

// --- setblock: basic block ---
{
  const cmd = buildSetblockCommand({ version: "java_1_21_11_plus", x: "~", y: "~", z: "~", block: "stone" });
  expect("setblock basic", cmd, "setblock ~ ~ ~ minecraft:stone");
}

// --- setblock: blockstate + mode ---
{
  const cmd = buildSetblockCommand({ version: "java_1_21_11_plus", x: "0", y: "64", z: "0", block: "oak_log", blockstate: "axis=x", mode: "keep" });
  expect("setblock blockstate+keep", cmd, "setblock 0 64 0 minecraft:oak_log[axis=x] keep");
}

// --- setblock: command_block NBT ---
{
  const cmd = buildSetblockCommand({
    version: "java_1_21_11_plus", x: "~", y: "~", z: "~", block: "command_block", blockstate: "facing=up",
    commandBlock: { command: "say hi", auto: true, trackOutput: false },
  });
  expect("setblock command_block nbt", cmd, `setblock ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi",auto:1b,TrackOutput:0b}`);
}

// --- setblock: chest Items ---
{
  const cmd = buildSetblockCommand({
    version: "java_1_21_11_plus", x: "~", y: "~", z: "~", block: "chest",
    containerItems: [{ slot: 0, item: { id: "diamond", count: 5 } }],
  });
  expect("setblock chest items", cmd, `setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:5}]}`);
}

// --- setblock: chest item with components ---
{
  const cmd = buildSetblockCommand({
    version: "java_1_21_11_plus", x: "~", y: "~", z: "~", block: "chest",
    containerItems: [{ slot: 0, item: { id: "stone", count: 1, components: { "minecraft:enchantment_glint_override": "1b" } } }],
  });
  expect("setblock chest item+components", cmd.includes('"minecraft:enchantment_glint_override":1b'), true);
  expect("setblock chest item+components count lowercase", cmd.includes("count:1"), true);
}

// --- setblock: withSlash ---
{
  const cmd = buildSetblockCommand({ version: "java_1_21_11_plus", x: "~", y: "~", z: "~", block: "stone", withSlash: true });
  expect("setblock withSlash", cmd, "/setblock ~ ~ ~ minecraft:stone");
}

// --- summon: basic ---
{
  const cmd = buildSummonCommand({ version: "java_1_21_11_plus", entityType: "pig" });
  expect("summon basic no pos", cmd, "summon minecraft:pig");
}

// --- summon: with coordinates ---
{
  const cmd = buildSummonCommand({ version: "java_1_21_11_plus", entityType: "pig", x: "~", y: "~", z: "~" });
  expect("summon with pos", cmd, "summon minecraft:pig ~ ~ ~");
}

// --- summon: NoAI + Silent + PersistenceRequired ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_11_plus", entityType: "zombie", x: "0", y: "64", z: "0",
    noAI: true, silent: true, persistenceRequired: true,
  });
  expect("summon flags", cmd, "summon minecraft:zombie 0 64 0 {NoAI:1b,Silent:1b,PersistenceRequired:1b}");
}

// --- summon: CustomName (SNBT string) ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_11_plus", entityType: "pig", x: "~", y: "~", z: "~",
    customName: [{ text: "Boss" }],
  });
  expect("summon customName snbt string", cmd.includes(`CustomName:'{"text":"Boss"}'`), true);
}

// --- summon: attributes modern (1.21.5+) ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_5", entityType: "zombie", x: "~", y: "~", z: "~",
    attributes: [{ id: "max_health", base: 40 }],
  });
  expect("summon attr modern key", cmd.includes('attributes:[{id:"minecraft:max_health",base:40d}]'), true);
}

// --- summon: attributes legacy (1.20.6) ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_1", entityType: "zombie", x: "~", y: "~", z: "~",
    attributes: [{ id: "max_health", base: 40 }],
  });
  expect("summon attr legacy key", cmd.includes('Attributes:[{Name:"minecraft:generic.max_health",Base:40d}]'), true);
}

// --- summon: active_effects ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_11_plus", entityType: "zombie", x: "~", y: "~", z: "~",
    effects: [{ id: "speed", duration: 200, amplifier: 1, showParticles: false }],
  });
  expect("summon effects", cmd.includes('active_effects:[{id:"minecraft:speed",duration:200,amplifier:1b,show_particles:0b}]'), true);
}

// --- summon: equipment modern ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_5", entityType: "zombie", x: "~", y: "~", z: "~",
    equipment: { mainhand: { id: "diamond_sword", count: 1 } },
  });
  expect("summon equipment modern", cmd.includes('equipment:{mainhand:{id:"minecraft:diamond_sword",count:1}}'), true);
}

// --- summon: HandItems legacy ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_1", entityType: "zombie", x: "~", y: "~", z: "~",
    equipment: { mainhand: { id: "diamond_sword", count: 1 } },
  });
  expect("summon equipment legacy HandItems", cmd.includes('HandItems:[{id:"minecraft:diamond_sword",count:1},{}]'), true);
}

// --- summon: Passengers ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_11_plus", entityType: "oak_boat", x: "~", y: "~", z: "~",
    passengers: [{ entityType: "chicken", noAI: true }],
  });
  expect("summon passengers", cmd.includes('Passengers:[{id:"minecraft:chicken",NoAI:1b}]'), true);
}

// --- summon: tags ---
{
  const cmd = buildSummonCommand({
    version: "java_1_21_11_plus", entityType: "pig", x: "~", y: "~", z: "~",
    tags: ["myTag", "anotherTag"],
  });
  expect("summon tags", cmd.includes('Tags:["myTag","anotherTag"]'), true);
}

// ====== say / effect / tp builders ======

import { buildSayCommand } from "./commands/say.ts";
import { buildEffectGiveCommand, buildEffectClearCommand } from "./commands/effect.ts";
import { buildTpCommand } from "./commands/tp.ts";

// ---- /say ----
{
  expect("say basic", buildSayCommand({ message: "hello world" }), "say hello world");
  expect("say selector", buildSayCommand({ message: "@a" }), "say @a");
  expect("say withSlash", buildSayCommand({ message: "hi", withSlash: true }), "/say hi");
}

// ---- /effect give ----
{
  // 最简：仅效果 id
  expect(
    "effect give minimal",
    buildEffectGiveCommand({ target: "@a", effect: "speed" }),
    "effect give @a minecraft:speed",
  );
  // 带时长
  expect(
    "effect give with duration",
    buildEffectGiveCommand({ target: "@a", effect: "minecraft:speed", duration: 30 }),
    "effect give @a minecraft:speed 30",
  );
  // 带等级
  expect(
    "effect give with amplifier",
    buildEffectGiveCommand({ target: "@a", effect: "speed", duration: 30, amplifier: 2 }),
    "effect give @a minecraft:speed 30 2",
  );
  // 隐藏粒子
  expect(
    "effect give hide particles",
    buildEffectGiveCommand({ target: "@a", effect: "speed", duration: 30, amplifier: 2, hideParticles: true }),
    "effect give @a minecraft:speed 30 2 true",
  );
  // 无限时长
  expect(
    "effect give infinite",
    buildEffectGiveCommand({ target: "@a", effect: "speed", duration: "infinite", amplifier: 1 }),
    "effect give @a minecraft:speed infinite 1",
  );
  // withSlash
  expect(
    "effect give withSlash",
    buildEffectGiveCommand({ target: "@a", effect: "speed", withSlash: true }),
    "/effect give @a minecraft:speed",
  );
  // amplifier 只有 0 时也要写出（因为后面可能有 hideParticles）
  expect(
    "effect give amplifier 0 with hideParticles",
    buildEffectGiveCommand({ target: "@a", effect: "speed", duration: 60, amplifier: 0, hideParticles: false }),
    "effect give @a minecraft:speed 60 0 false",
  );
}

// ---- /effect clear ----
{
  expect(
    "effect clear all",
    buildEffectClearCommand({ target: "@a" }),
    "effect clear @a",
  );
  expect(
    "effect clear one",
    buildEffectClearCommand({ target: "@a", effect: "speed" }),
    "effect clear @a minecraft:speed",
  );
  expect(
    "effect clear withSlash",
    buildEffectClearCommand({ target: "@a", withSlash: true }),
    "/effect clear @a",
  );
}

// ---- /tp ----
{
  // 绝对坐标
  expect(
    "tp absolute coords",
    buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0" }),
    "tp @s 0 64 0",
  );
  // 相对坐标
  expect(
    "tp relative coords",
    buildTpCommand({ targets: "@s", x: "~", y: "~", z: "~" }),
    "tp @s ~ ~ ~",
  );
  // 本地坐标
  expect(
    "tp local coords",
    buildTpCommand({ targets: "@s", x: "^", y: "^", z: "^1" }),
    "tp @s ^ ^ ^1",
  );
  // 带旋转角
  expect(
    "tp with rotation",
    buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", yRot: "90", xRot: "45" }),
    "tp @s 0 64 0 90 45",
  );
  // facing 坐标
  expect(
    "tp facing coords",
    buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", facingX: "10", facingY: "70", facingZ: "10" }),
    "tp @s 0 64 0 facing 10 70 10",
  );
  // 传送到实体
  expect(
    "tp to entity",
    buildTpCommand({ targets: "@s", destination: "@e[type=pig,limit=1]" }),
    "tp @s @e[type=pig,limit=1]",
  );
  // teleport 别名
  expect(
    "teleport alias",
    buildTpCommand({ targets: "@s", x: "0", y: "64", z: "0", useTeleportAlias: true }),
    "teleport @s 0 64 0",
  );
  // withSlash
  expect(
    "tp withSlash",
    buildTpCommand({ targets: "@s", x: "~", y: "~", z: "~", withSlash: true }),
    "/tp @s ~ ~ ~",
  );
}

// --- summary ---
console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
