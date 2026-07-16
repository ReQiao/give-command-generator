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

// --- 36. font emitted (modern) ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[{ text: "A", font: "minecraft:illageralt" }]];
  const cmd = buildGiveCommand(f);
  expect("font emitted", cmd.includes('"font":"minecraft:illageralt"'), true);
}

// --- 37. obfuscated emitted ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[{ text: "A", obfuscated: true }]];
  const cmd = buildGiveCommand(f);
  expect("obfuscated emitted", cmd.includes('"obfuscated":true'), true);
}

// --- 38. named color passthrough ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[{ text: "A", color: "red" }]];
  const cmd = buildGiveCommand(f);
  expect("named color emitted", cmd.includes('"color":"red"'), true);
}

// --- 39. object sprite + player on 1.21.11+ ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[
    { type: "object", object: "atlas", atlas: "minecraft:blocks", sprite: "block/stone" },
    { type: "object", object: "player", player: "Notch", hat: true },
  ]];
  const cmd = buildGiveCommand(f);
  expect("object sprite present", cmd.includes('{"type":"object","object":"atlas","atlas":"minecraft:blocks","sprite":"block/stone"}'), true);
  expect("object player present", cmd.includes('{"type":"object","object":"player","player":"Notch","hat":true}'), true);
}

// --- 40. object stripped on 1.21.6 (<1.21.9) with warning ---
{
  const f = base("java_1_21_6");
  f.displayName = [[{ text: "x" }, { type: "object", sprite: "item/diamond" }]];
  const warnings = [];
  const cmd = buildGiveCommand(f, warnings);
  expect("1.21.6 object stripped", !cmd.includes('"type":"object"'), true);
  expect("1.21.6 object warning", warnings.length > 0, true);
  expect("1.21.6 keeps text", cmd.includes('{"text":"x"}'), true);
}

// --- 41. object stripped on 1.21 legacy too ---
{
  const f = base("java_1_21");
  f.displayName = [[{ type: "object", sprite: "item/diamond" }]];
  const cmd = buildGiveCommand(f);
  expect("1.21 object stripped", !cmd.includes("object"), true);
}

// --- 42. click_event modern snake_case (1.21.5+) ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[{ text: "c", click_event: { action: "run_command", value: "say hi" } }]];
  const cmd = buildGiveCommand(f);
  expect("modern click_event snake_case", cmd.includes('"click_event":{"action":"run_command","command":"say hi"}'), true);
}

// --- 43. click_event legacy camelCase (1.21) ---
{
  const f = base("java_1_21");
  f.displayName = [[{ text: "c", click_event: { action: "run_command", value: "/say hi" } }]];
  const cmd = buildGiveCommand(f);
  expect("legacy clickEvent camelCase", cmd.includes('"clickEvent":{"action":"run_command","value":"/say hi"}'), true);
}

// --- 44. hover_event modern show_text ---
{
  const f = base("java_1_21_11_plus");
  f.displayName = [[{ text: "h", hover_event: { action: "show_text", text: [{ text: "tip" }] } }]];
  const cmd = buildGiveCommand(f);
  expect("modern hover_event value", cmd.includes('"hover_event":{"action":"show_text","value":[{"text":"tip"}]}'), true);
}

// --- 45. hover_event legacy contents ---
{
  const f = base("java_1_21");
  f.displayName = [[{ text: "h", hover_event: { action: "show_text", text: [{ text: "tip" }] } }]];
  const cmd = buildGiveCommand(f);
  expect("legacy hoverEvent contents", cmd.includes('"hoverEvent":{"action":"show_text","contents":[{"text":"tip"}]}'), true);
}

// --- 46. translatable with args ---
{
  const f = base("java_1_21_11_plus");
  f.lore = [[{ type: "translatable", translate: "item.minecraft.stone", with: [{ text: "arg" }] }]];
  const cmd = buildGiveCommand(f);
  expect("translatable emitted", cmd.includes('"translate":"item.minecraft.stone"'), true);
  expect("translatable with args", cmd.includes('"with":[{"text":"arg"}]'), true);
}

// --- 47. keybind / selector / score / nbt ---
{
  const f = base("java_1_21_11_plus");
  f.lore = [
    [{ type: "keybind", keybind: "key.jump" }],
    [{ type: "selector", selector: "@p" }],
    [{ type: "score", score: { name: "@s", objective: "kills" } }],
    [{ type: "nbt", nbt: "Health", source: "entity", entity: "@s", interpret: true }],
  ];
  const cmd = buildGiveCommand(f);
  expect("keybind emitted", cmd.includes('{"keybind":"key.jump"}'), true);
  expect("selector emitted", cmd.includes('{"selector":"@p"}'), true);
  expect("score emitted", cmd.includes('{"score":{"name":"@s","objective":"kills"}}'), true);
  expect("nbt emitted", cmd.includes('"nbt":"Health","source":"entity","entity":"@s","interpret":true'), true);
}

// --- 48. shadow_color array kept on 1.21.4+, converted to int below ---
{
  const f = base("java_1_21_4");
  f.displayName = [[{ text: "s", shadow_color: [1, 0, 0, 1] }]];
  const cmd = buildGiveCommand(f);
  expect("1.21.4 shadow array kept", cmd.includes('"shadow_color":[1,0,0,1]'), true);
}
{
  const f = base("java_1_21_2");
  f.displayName = [[{ text: "s", shadow_color: [1, 0, 0, 1] }]];
  const cmd = buildGiveCommand(f);
  expect("1.21.2 shadow array -> int", cmd.includes('"shadow_color":-65536'), true);
}

// --- summary ---
console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
