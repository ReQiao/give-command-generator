/**
 * give 表单的默认值与归一化（来自外部存储/模板的脏数据 → 合法 GiveForm）。
 */

import type { GiveForm } from "./types";
import { normalizeInt, normalizeNumber } from "./util";
import { normalizeVersion } from "./version";

export function createDefaultForm(): GiveForm {
  return {
    version: "java_1_21_11_plus",
    target: "@a",
    item: "石头",
    itemSearch: "",
    count: 1,
    withSlash: false,
    templateName: "未命名模板",
    bedrockDataValue: 0,
    bedrockItemLock: "不设置",
    bedrockKeepOnDeath: false,
    displayName: [],
    itemName: [],
    lore: [],
    rarity: "不设置",
    glint: "默认",
    enchantments: [],
    attributes: [],
    blockLimits: [],
    unbreakable: false,
    glider: false,
    deathProtection: false,
    deathEffects: [],
    damageEnabled: false,
    damage: 0,
    maxDamageEnabled: false,
    maxDamage: 1,
    stackEnabled: false,
    maxStackSize: 1,
    repairEnabled: false,
    repairCost: 0,
    hiddenComponents: "",
    foodEnabled: false,
    nutrition: 0,
    saturation: 0,
    alwaysEat: "默认",
    consumableEnabled: false,
    consumeSeconds: 0,
    consumeSound: "",
    consumeParticles: "默认",
    consumeEffects: [],
    toolEnabled: false,
    defaultMiningSpeed: 1,
    damagePerBlock: 0,
    toolRules: [],
  };
}

export function normalizeForm(value: unknown): GiveForm {
  const fallback = createDefaultForm();
  if (!value || typeof value !== "object") return fallback;
  const data = value as Partial<GiveForm>;

  return {
    ...fallback,
    ...data,
    version: normalizeVersion(data.version),
    count: normalizeInt(data.count, fallback.count, 1),
    bedrockDataValue: normalizeInt(data.bedrockDataValue, fallback.bedrockDataValue, 0),
    damage: normalizeInt(data.damage, fallback.damage, 0),
    maxDamage: normalizeInt(data.maxDamage, fallback.maxDamage, 1),
    maxStackSize: normalizeInt(data.maxStackSize, fallback.maxStackSize, 1),
    repairCost: normalizeInt(data.repairCost, fallback.repairCost, 0),
    nutrition: normalizeInt(data.nutrition, fallback.nutrition, 0),
    saturation: normalizeNumber(data.saturation, fallback.saturation, 0),
    consumeSeconds: normalizeNumber(data.consumeSeconds, fallback.consumeSeconds, 0),
    defaultMiningSpeed: normalizeNumber(data.defaultMiningSpeed, fallback.defaultMiningSpeed, 0),
    damagePerBlock: normalizeInt(data.damagePerBlock, fallback.damagePerBlock, 0),
    enchantments: Array.isArray(data.enchantments) ? data.enchantments : [],
    attributes: Array.isArray(data.attributes) ? data.attributes : [],
    blockLimits: Array.isArray(data.blockLimits) ? data.blockLimits : [],
    deathEffects: Array.isArray(data.deathEffects) ? data.deathEffects : [],
    consumeEffects: Array.isArray(data.consumeEffects) ? data.consumeEffects : [],
    toolRules: Array.isArray(data.toolRules) ? data.toolRules : [],
    displayName: Array.isArray(data.displayName) ? data.displayName : [],
    itemName: Array.isArray(data.itemName) ? data.itemName : [],
    lore: Array.isArray(data.lore) ? data.lore : [],
  };
}
