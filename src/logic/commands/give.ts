/**
 * /give 指令构建器。
 *
 * 按版本族分派：
 *   - bedrock                → buildBedrock（JSON 组件对象）
 *   - 1.21 / 1.21.1 (legacy) → buildJava121Legacy（enchantments 带 levels、属性带 modifiers 外层）
 *   - 1.20.5 / 1.21.2 / 1.21.5+ → buildModernFamily（同一逻辑，差异由 profile 控制）
 */

import {
  ATTRIBUTES,
  BLOCKS,
  CORRECT_FOR_DROPS,
  ENCHANTS,
  ITEM_LOCK_MODES,
  ITEMS,
  LIMIT_TYPES,
  OPERATIONS,
  RARITIES,
  SLOTS,
} from "../../data/catalog";
import type { BlockLimitRow, EffectGroup, GiveForm, ToolRuleRow } from "../types";
import {
  boolByte,
  compact,
  componentId,
  fmtNumber,
  namespaced,
  percentToProbability,
  quote,
  snbtJsonString,
  splitCsv,
} from "../snbt";
import { mapCatalog, pairValue } from "../catalog-util";
import { cryptoId, normalizeInt, normalizeTarget } from "../util";
import {
  JAVA_1_20_5_PROFILE,
  JAVA_1_21_2_PROFILE,
  MODERN_PROFILE,
  type ModernProfile,
  isJava1205Family,
  isJava1212Family,
  isJava121LegacyFamily,
} from "../version";

export function buildGiveCommand(form: GiveForm): string {
  if (form.version === "bedrock") {
    return buildBedrock(form);
  }
  if (isJava121LegacyFamily(form.version)) {
    return buildJava121Legacy(form);
  }
  if (isJava1205Family(form.version)) {
    return buildModernFamily(form, JAVA_1_20_5_PROFILE);
  }
  if (isJava1212Family(form.version)) {
    return buildModernFamily(form, JAVA_1_21_2_PROFILE);
  }

  return buildModernFamily(form, MODERN_PROFILE);
}

function buildModernFamily(form: GiveForm, profile: ModernProfile): string {
  const parts: string[] = [];
  const add = (name: string, value: string) => parts.push(`${name}=${value}`);

  if (form.displayName.length) {
    add("custom_name", profile.textAsSnbtString ? snbtJsonString(form.displayName[0] ?? []) : compact(form.displayName[0] ?? []));
  }
  if (form.itemName.length) {
    add("item_name", profile.textAsSnbtString ? snbtJsonString(form.itemName[0] ?? []) : compact(form.itemName[0] ?? []));
  }
  if (form.lore.length) {
    add("lore", profile.textAsSnbtString ? `[${form.lore.map((line) => snbtJsonString(line)).join(",")}]` : compact(form.lore));
  }

  const rarity = pairValue(RARITIES, form.rarity);
  if (rarity !== "none") add("rarity", rarity);

  if (form.glint !== "默认") {
    add("enchantment_glint_override", form.glint === "开启" ? "true" : "false");
  }

  const enchants = form.enchantments
    .filter((row) => String(row.id ?? "").trim())
    .map((row) => `${componentId(mapCatalog(ENCHANTS, row.id))}:${normalizeInt(row.level, 1, 1)}`);
  if (enchants.length) add("enchantments", `{${enchants.join(",")}}`);

  if (profile.supportsAttributeModifiers) {
    const attributes = form.attributes
      .filter((row) => String(row.type ?? "").trim())
      .map((row) => {
        const fields = [
          `type:${componentId(mapCatalog(ATTRIBUTES, row.type))}`,
          `amount:${fmtNumber(row.amount)}`,
        ];
        const slot = pairValue(SLOTS, row.slot || "任意");
        if (slot && slot !== "any") fields.push(`slot:${slot}`);
        fields.push(`id:${quote(row.id || cryptoId())}`);
        fields.push(`operation:${pairValue(OPERATIONS, row.operation || "加算")}`);
        return `{${fields.join(",")}}`;
      });
    if (attributes.length) add("attribute_modifiers", `[${attributes.join(",")}]`);
  }

  const place = form.blockLimits.filter((row) => ["place", "both"].includes(pairValue(LIMIT_TYPES, row.type)));
  const brk = form.blockLimits.filter((row) => ["break", "both"].includes(pairValue(LIMIT_TYPES, row.type)));
  const placePredicates = blockPredicateList(place);
  const breakPredicates = blockPredicateList(brk);
  const wrapPredicates = (preds: string[]) =>
    profile.adventurePredicateWrapper ? `{predicates:[${preds.join(",")}]}` : `[${preds.join(",")}]`;
  if (placePredicates.length) add("can_place_on", wrapPredicates(placePredicates));
  if (breakPredicates.length) add("can_break", wrapPredicates(breakPredicates));

  if (form.unbreakable) add("unbreakable", "{}");
  if (profile.supportsGlider && form.glider) add("glider", "{}");

  if (profile.supportsDeathProtection) {
    const deathEffects = buildEffectGroups(form.deathEffects);
    if (form.deathProtection || deathEffects) {
      add("death_protection", deathEffects ? `{death_effects:[${deathEffects}]}` : "{}");
    }
  }

  if (form.damageEnabled) add("damage", String(normalizeInt(form.damage, 0, 0)));
  if (form.maxDamageEnabled) add("max_damage", String(normalizeInt(form.maxDamage, 1, 1)));
  if (form.stackEnabled) add("max_stack_size", String(normalizeInt(form.maxStackSize, 1, 1)));
  if (form.repairEnabled) add("repair_cost", String(normalizeInt(form.repairCost, 0, 0)));

  if (profile.supportsTooltipDisplay) {
    const hidden = splitCsv(form.hiddenComponents);
    if (hidden.length) add("tooltip_display", `{hidden_components:[${hidden.map((value) => quote(namespaced(value))).join(",")}]}`);
  }

  if (form.foodEnabled) {
    const fields = [
      `nutrition:${normalizeInt(form.nutrition, 0, 0)}`,
      `saturation:${fmtNumber(form.saturation)}`,
    ];
    if (form.alwaysEat !== "默认") fields.push(`can_always_eat:${form.alwaysEat === "是" ? "1b" : "0b"}`);
    add("food", `{${fields.join(",")}}`);
  }

  if (profile.supportsConsumable) {
    const consumeEffects = buildEffectGroups(form.consumeEffects);
    if (form.consumableEnabled || consumeEffects) {
      const fields: string[] = [];
      if (form.consumableEnabled) {
        fields.push(`consume_seconds:${fmtNumber(form.consumeSeconds)}`);
        if (form.consumeSound.trim()) fields.push(`sound:${quote(namespaced(form.consumeSound))}`);
        if (form.consumeParticles !== "默认") {
          fields.push(`has_consume_particles:${form.consumeParticles === "是" ? "1b" : "0b"}`);
        }
      }
      if (consumeEffects) fields.push(`on_consume_effects:[${consumeEffects}]`);
      add("consumable", `{${fields.join(",")}}`);
    }
  }

  const toolRules = buildToolRules(form.toolRules);
  if (form.toolEnabled || toolRules) {
    const fields: string[] = [];
    if (form.toolEnabled) {
      fields.push(`default_mining_speed:${fmtNumber(form.defaultMiningSpeed)}`);
      fields.push(`damage_per_block:${normalizeInt(form.damagePerBlock, 0, 0)}`);
    }
    if (toolRules) fields.push(`rules:[${toolRules}]`);
    add("tool", `{${fields.join(",")}}`);
  }

  const body = parts.length ? `[${parts.join(",")}]` : "";
  const slash = form.withSlash ? "/" : "";
  return `${slash}give ${normalizeTarget(form.target)} ${mapCatalog(ITEMS, form.item)}${body} ${normalizeInt(form.count, 1, 1)}`;
}

function buildJava121Legacy(form: GiveForm): string {
  const parts: string[] = [];
  const add = (name: string, value: string) => parts.push(`${name}=${value}`);

  if (form.displayName.length) add("custom_name", snbtJsonString(form.displayName[0] ?? []));
  if (form.itemName.length) add("item_name", snbtJsonString(form.itemName[0] ?? []));
  if (form.lore.length) add("lore", `[${form.lore.map((line) => snbtJsonString(line)).join(",")}]`);

  const rarity = pairValue(RARITIES, form.rarity);
  if (rarity !== "none") add("rarity", rarity);

  if (form.glint !== "默认") {
    add("enchantment_glint_override", form.glint === "开启" ? "true" : "false");
  }

  const enchants = form.enchantments
    .filter((row) => String(row.id ?? "").trim())
    .map((row) => `${componentId(mapCatalog(ENCHANTS, row.id))}:${normalizeInt(row.level, 1, 1)}`);
  if (enchants.length) add("enchantments", `{levels:{${enchants.join(",")}}}`);

  const attributes = form.attributes
    .filter((row) => String(row.type ?? "").trim())
    .map((row) => {
      const fields = [
        `type:${quote(legacyAttributeType(row.type))}`,
        `amount:${fmtNumber(row.amount)}`,
      ];
      const slot = pairValue(SLOTS, row.slot || "any");
      if (slot && slot !== "any") fields.push(`slot:${slot}`);
      fields.push(`id:${quote(row.id || cryptoId())}`);
      fields.push(`operation:${pairValue(OPERATIONS, row.operation || "add_value")}`);
      return `{${fields.join(",")}}`;
    });
  if (attributes.length) add("attribute_modifiers", `{modifiers:[${attributes.join(",")}]}`);

  const place = form.blockLimits.filter((row) => ["place", "both"].includes(pairValue(LIMIT_TYPES, row.type)));
  const brk = form.blockLimits.filter((row) => ["break", "both"].includes(pairValue(LIMIT_TYPES, row.type)));
  const placePredicates = blockPredicateList(place);
  const breakPredicates = blockPredicateList(brk);
  if (placePredicates.length) add("can_place_on", `{predicates:[${placePredicates.join(",")}]}`);
  if (breakPredicates.length) add("can_break", `{predicates:[${breakPredicates.join(",")}]}`);

  if (form.unbreakable) add("unbreakable", "{}");

  if (form.damageEnabled) add("damage", String(normalizeInt(form.damage, 0, 0)));
  if (form.maxDamageEnabled) add("max_damage", String(normalizeInt(form.maxDamage, 1, 1)));
  if (form.stackEnabled) add("max_stack_size", String(normalizeInt(form.maxStackSize, 1, 1)));
  if (form.repairEnabled) add("repair_cost", String(normalizeInt(form.repairCost, 0, 0)));

  const legacyFood = buildJava121Food(form);
  if (legacyFood) add("food", legacyFood);

  const toolRules = buildToolRules(form.toolRules);
  if (form.toolEnabled || toolRules) {
    const fields: string[] = [];
    if (form.toolEnabled) {
      fields.push(`default_mining_speed:${fmtNumber(form.defaultMiningSpeed)}`);
      fields.push(`damage_per_block:${normalizeInt(form.damagePerBlock, 0, 0)}`);
    }
    if (toolRules) fields.push(`rules:[${toolRules}]`);
    add("tool", `{${fields.join(",")}}`);
  }

  const body = parts.length ? `[${parts.join(",")}]` : "";
  const slash = form.withSlash ? "/" : "";
  return `${slash}give ${normalizeTarget(form.target)} ${mapCatalog(ITEMS, form.item)}${body} ${normalizeInt(form.count, 1, 1)}`;
}

function buildBedrock(form: GiveForm): string {
  const components: Record<string, unknown> = {};
  const place = form.blockLimits
    .filter((row) => ["place", "both"].includes(pairValue(LIMIT_TYPES, row.type)) && String(row.block ?? "").trim())
    .map((row) => componentId(mapCatalog(BLOCKS, row.block)));
  const brk = form.blockLimits
    .filter((row) => ["break", "both"].includes(pairValue(LIMIT_TYPES, row.type)) && String(row.block ?? "").trim())
    .map((row) => componentId(mapCatalog(BLOCKS, row.block)));

  if (place.length) components["minecraft:can_place_on"] = { blocks: place };
  if (brk.length) components["minecraft:can_destroy"] = { blocks: brk };

  const lockMode = pairValue(ITEM_LOCK_MODES, form.bedrockItemLock);
  if (lockMode !== "none") components["minecraft:item_lock"] = { mode: lockMode };
  if (form.bedrockKeepOnDeath) components["minecraft:keep_on_death"] = {};

  const suffix = Object.keys(components).length ? ` ${compact(components)}` : "";
  const slash = form.withSlash ? "/" : "";
  return `${slash}give ${normalizeTarget(form.target)} ${componentId(mapCatalog(ITEMS, form.item))} ${normalizeInt(form.count, 1, 1)} ${normalizeInt(form.bedrockDataValue, 0, 0)}${suffix}`;
}

function buildJava121Food(form: GiveForm): string {
  const foodEffects = buildJava121FoodEffects(form.consumeEffects);
  if (!form.foodEnabled && !form.consumableEnabled && !foodEffects) return "";

  const fields = [
    `nutrition:${normalizeInt(form.nutrition, 0, 0)}`,
    `saturation:${fmtNumber(form.saturation)}`,
  ];
  if (form.alwaysEat !== "默认") fields.push(`can_always_eat:${form.alwaysEat === "是" ? "1b" : "0b"}`);
  if (form.consumableEnabled) fields.push(`eat_seconds:${fmtNumber(form.consumeSeconds)}`);
  if (foodEffects) fields.push(`effects:[${foodEffects}]`);
  return `{${fields.join(",")}}`;
}

function buildJava121FoodEffects(groups: EffectGroup[]): string {
  const out: string[] = [];
  for (const group of groups || []) {
    if (group.type !== "apply_effects") continue;
    const probability = `${percentToProbability(group.probability_percent ?? 100)}f`;
    for (const effect of group.effects || []) {
      const id = componentId(typeof effect === "string" ? effect : effect.id);
      if (!id) continue;
      const fields = [`id:${id}`];
      if (typeof effect !== "string") {
        fields.push(`duration:${normalizeInt(effect.duration, 0, 0)}`);
        fields.push(`amplifier:${normalizeInt(effect.amplifier, 0, 0)}`);
        fields.push(`ShowParticles:${boolByte(effect.show_particles ?? true)}`);
        fields.push(`ShowIcon:${boolByte(effect.show_icon ?? true)}`);
      }
      out.push(`{effect:{${fields.join(",")}},probability:${probability}}`);
    }
  }
  return out.join(",");
}

function buildEffectGroups(groups: EffectGroup[]): string {
  const out: string[] = [];
  for (const group of groups || []) {
    const type = group.type;
    if (type === "apply_effects") {
      const effects = (group.effects || [])
        .map((effect) => {
          if (typeof effect === "string") return null;
          const id = componentId(effect.id);
          if (!id) return null;
          const duration = normalizeInt(effect.duration, 0, 0);
          const amplifier = normalizeInt(effect.amplifier, 0, 0);
          const particles = boolByte(effect.show_particles ?? true);
          const icon = boolByte(effect.show_icon ?? true);
          return `{id:${id},duration:${duration},amplifier:${amplifier},ShowParticles:${particles},ShowIcon:${icon}}`;
        })
        .filter(Boolean);
      if (effects.length) {
        out.push(`{type:apply_effects,probability:${percentToProbability(group.probability_percent ?? 100)},effects:[${effects.join(",")}]}`);
      }
    } else if (type === "remove_effects") {
      const effects = (group.effects || [])
        .map((effect) => componentId(typeof effect === "string" ? effect : effect.id))
        .filter(Boolean);
      if (effects.length) out.push(`{type:remove_effects,effects:[${effects.join(",")}]}`);
    } else if (type === "clear_all_effects") {
      out.push("{type:clear_all_effects}");
    } else if (type === "teleport_randomly") {
      out.push(`{type:teleport_randomly,diameter:${fmtNumber(group.diameter ?? 16)}}`);
    }
  }
  return out.join(",");
}

function buildToolRules(rules: ToolRuleRow[]): string {
  const out: string[] = [];
  for (const rule of rules || []) {
    const rawBlocks = Array.isArray(rule.blocks) ? rule.blocks : splitCsv(rule.blocks);
    const blocks = rawBlocks.map((block) => componentId(mapCatalog(BLOCKS, block))).filter(Boolean);
    if (!blocks.length) continue;
    const fields = [`blocks:[${blocks.join(",")}]`];
    if (String(rule.speed ?? "").trim()) fields.push(`speed:${fmtNumber(rule.speed)}f`);
    const correct = pairValue(CORRECT_FOR_DROPS, rule.correct_for_drops);
    if (correct === "true") fields.push("correct_for_drops:1b");
    if (correct === "false") fields.push("correct_for_drops:0b");
    out.push(`{${fields.join(",")}}`);
  }
  return out.join(",");
}

function blockPredicateList(rows: BlockLimitRow[]): string[] {
  return rows
    .filter((row) => String(row.block ?? "").trim())
    .map((row) => `{blocks:${quote(mapCatalog(BLOCKS, row.block))}}`);
}

function legacyAttributeType(value: string): string {
  const id = componentId(mapCatalog(ATTRIBUTES, value));
  if (id.includes(".")) return id;
  const mapped: Record<string, string> = {
    armor: "generic.armor",
    armor_toughness: "generic.armor_toughness",
    attack_damage: "generic.attack_damage",
    attack_knockback: "generic.attack_knockback",
    attack_speed: "generic.attack_speed",
    block_break_speed: "generic.block_break_speed",
    block_interaction_range: "player.block_interaction_range",
    entity_interaction_range: "player.entity_interaction_range",
    fall_damage_multiplier: "generic.fall_damage_multiplier",
    knockback_resistance: "generic.knockback_resistance",
    luck: "generic.luck",
    max_absorption: "generic.max_absorption",
    max_health: "generic.max_health",
    mining_efficiency: "player.mining_efficiency",
    oxygen_bonus: "generic.oxygen_bonus",
    safe_fall_distance: "generic.safe_fall_distance",
    sneaking_speed: "player.sneaking_speed",
    submerged_mining_speed: "player.submerged_mining_speed",
    water_movement_efficiency: "generic.water_movement_efficiency",
  };
  return mapped[id] ?? `generic.${id}`;
}
