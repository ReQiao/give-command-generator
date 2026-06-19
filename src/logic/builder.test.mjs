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

// --- summary ---
console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
