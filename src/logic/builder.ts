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
  type CatalogRow,
  type PairRow,
} from "../data/catalog";

export type GiveVersion =
  | "java_1_20_5"
  | "java_1_21"
  | "java_1_21_1"
  | "java_1_21_2"
  | "java_1_21_3"
  | "java_1_21_4"
  | "java_1_21_5"
  | "java_1_21_6"
  | "java_1_21_9"
  | "java_1_21_11_plus"
  | "java_26_1"
  | "java_26_2_plus"
  | "bedrock";

export interface TextComponent {
  text: string;
  bold?: boolean;
  italic?: boolean;
  underlined?: boolean;
  strikethrough?: boolean;
  color?: string;
  shadow_color?: number;
}

export type RichLine = TextComponent[];

export interface EnchantRow {
  id: string;
  level: number | string;
}

export interface AttributeRow {
  type: string;
  amount: number | string;
  slot: string;
  operation: string;
  id: string;
}

export interface BlockLimitRow {
  block: string;
  type: string;
}

export interface EffectItem {
  id: string;
  duration?: number | string;
  amplifier?: number | string;
  show_particles?: boolean;
  show_icon?: boolean;
}

export interface EffectGroup {
  type: "apply_effects" | "remove_effects" | "clear_all_effects" | "teleport_randomly";
  probability_percent?: number | string;
  diameter?: number | string;
  effects?: Array<EffectItem | string>;
}

export interface ToolRuleRow {
  blocks: string[] | string;
  speed: number | string;
  correct_for_drops: string;
}

export interface GiveForm {
  version: GiveVersion;
  target: string;
  item: string;
  itemSearch: string;
  count: number;
  withSlash: boolean;
  templateName: string;
  bedrockDataValue: number;
  bedrockItemLock: string;
  bedrockKeepOnDeath: boolean;
  displayName: RichLine[];
  itemName: RichLine[];
  lore: RichLine[];
  rarity: string;
  glint: string;
  enchantments: EnchantRow[];
  attributes: AttributeRow[];
  blockLimits: BlockLimitRow[];
  unbreakable: boolean;
  glider: boolean;
  deathProtection: boolean;
  deathEffects: EffectGroup[];
  damageEnabled: boolean;
  damage: number;
  maxDamageEnabled: boolean;
  maxDamage: number;
  stackEnabled: boolean;
  maxStackSize: number;
  repairEnabled: boolean;
  repairCost: number;
  hiddenComponents: string;
  foodEnabled: boolean;
  nutrition: number;
  saturation: number;
  alwaysEat: string;
  consumableEnabled: boolean;
  consumeSeconds: number;
  consumeSound: string;
  consumeParticles: string;
  consumeEffects: EffectGroup[];
  toolEnabled: boolean;
  defaultMiningSpeed: number;
  damagePerBlock: number;
  toolRules: ToolRuleRow[];
}

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

interface ModernProfile {
  textAsSnbtString: boolean;
  adventurePredicateWrapper: boolean;
  supportsTooltipDisplay: boolean;
  supportsConsumable: boolean;
  supportsGlider: boolean;
  supportsDeathProtection: boolean;
  supportsAttributeModifiers: boolean;
}

const MODERN_PROFILE: ModernProfile = {
  textAsSnbtString: false,
  adventurePredicateWrapper: false,
  supportsTooltipDisplay: true,
  supportsConsumable: true,
  supportsGlider: true,
  supportsDeathProtection: true,
  supportsAttributeModifiers: true,
};

const JAVA_1_21_2_PROFILE: ModernProfile = {
  textAsSnbtString: true,
  adventurePredicateWrapper: true,
  supportsTooltipDisplay: false,
  supportsConsumable: true,
  supportsGlider: true,
  supportsDeathProtection: true,
  supportsAttributeModifiers: true,
};

const JAVA_1_20_5_PROFILE: ModernProfile = {
  textAsSnbtString: true,
  adventurePredicateWrapper: true,
  supportsTooltipDisplay: false,
  supportsConsumable: false,
  supportsGlider: false,
  supportsDeathProtection: false,
  supportsAttributeModifiers: false,
};

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

  if (form.glint !== "\u9ed8\u8ba4") {
    add("enchantment_glint_override", form.glint === "\u5f00\u542f" ? "true" : "false");
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
  if (form.alwaysEat !== "\u9ed8\u8ba4") fields.push(`can_always_eat:${form.alwaysEat === "\u662f" ? "1b" : "0b"}`);
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

export function compact(value: unknown): string {
  return JSON.stringify(value);
}

function snbtJsonString(value: unknown): string {
  return `'${JSON.stringify(value).replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}

export function quote(value: string): string {
  return JSON.stringify(value);
}

export function namespaced(value: string, namespace = "minecraft"): string {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return text.includes(":") ? text : `${namespace}:${text}`;
}

export function stripMinecraftNamespace(value: string): string {
  const text = String(value ?? "").trim();
  return text.startsWith("minecraft:") ? text.slice("minecraft:".length) : text;
}

export function componentId(value: string): string {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return stripMinecraftNamespace(namespaced(text));
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

export function boolByte(value: boolean): string {
  return value ? "1b" : "0b";
}

export function fmtNumber(value: unknown): string {
  const num = Number(value);
  if (!Number.isFinite(num)) return String(value ?? "").trim();
  if (Number.isInteger(num)) return String(num);
  return num.toFixed(10).replace(/0+$/, "").replace(/\.$/, "");
}

export function percentToProbability(value: unknown): string {
  const num = Math.max(0, Math.min(100, Number(value) || 0));
  return fmtNumber(num / 100);
}

export function splitCsv(value: string | string[]): string[] {
  if (Array.isArray(value)) return value.map((item) => item.trim()).filter(Boolean);
  return String(value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function mapCatalog(catalog: readonly CatalogRow[], text: string): string {
  for (const row of catalog) {
    if (text === row[0] || text === row[1]) return row[0];
  }
  return namespaced(text);
}

export function displayList(catalog: readonly CatalogRow[]): string[] {
  return catalog.map((row) => row[1]);
}

export function matches(row: readonly unknown[], query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return row.join(" ").toLowerCase().includes(q);
}

export function pairValue(pairs: readonly PairRow[], text: string): string {
  for (const [label, value] of pairs) {
    if (text === label || text === value) return value;
  }
  return text;
}

export function pairText(pairs: readonly PairRow[], value: string): string {
  for (const [label, itemValue] of pairs) {
    if (value === itemValue || value === label) return label;
  }
  return value;
}

export function hexToRgb(value: string): [number, number, number] {
  const text = value.trim();
  if (!/^#[0-9a-fA-F]{6}$/.test(text)) throw new Error("颜色必须是 #RRGGBB");
  return [
    Number.parseInt(text.slice(1, 3), 16),
    Number.parseInt(text.slice(3, 5), 16),
    Number.parseInt(text.slice(5, 7), 16),
  ];
}

export function rgbToHex(value: [number, number, number]): string {
  return `#${value.map((item) => item.toString(16).padStart(2, "0")).join("")}`;
}

export function colorLerp(start: string, end: string, count: number): string[] {
  if (count <= 0) return [];
  const a = hexToRgb(start);
  const b = hexToRgb(end);
  if (count === 1) return [rgbToHex(a)];
  return Array.from({ length: count }, (_, index) => {
    const ratio = index / (count - 1);
    return rgbToHex([
      Math.round(a[0] + (b[0] - a[0]) * ratio),
      Math.round(a[1] + (b[1] - a[1]) * ratio),
      Math.round(a[2] + (b[2] - a[2]) * ratio),
    ]);
  });
}

export function shadowColorInt(hexColor: string, alphaPercent: number): number {
  const [r, g, b] = hexToRgb(hexColor);
  const alpha = Math.round(Math.max(0, Math.min(100, alphaPercent)) / 100 * 255);
  const value = (alpha << 24) | (r << 16) | (g << 8) | b;
  return value >= 2 ** 31 ? value - 2 ** 32 : value;
}

function normalizeTarget(value: string): string {
  const text = String(value ?? "").trim();
  return text.length > 0 ? text : "@a";
}

function normalizeVersion(value: unknown): GiveVersion {
  const text = String(value ?? "").trim();
  if (
    text === "java_1_20_5" ||
    text === "java_1_21" ||
    text === "java_1_21_1" ||
    text === "java_1_21_2" ||
    text === "java_1_21_3" ||
    text === "java_1_21_4" ||
    text === "java_1_21_5" ||
    text === "java_1_21_6" ||
    text === "java_1_21_9" ||
    text === "java_1_21_11_plus" ||
    text === "java_26_1" ||
    text === "java_26_2_plus" ||
    text === "bedrock"
  ) {
    return text;
  }
  return "java_1_21_11_plus";
}

export function isJava121LegacyFamily(version: GiveVersion): boolean {
  return version === "java_1_21" || version === "java_1_21_1";
}

export function isJava1205Family(version: GiveVersion): boolean {
  return version === "java_1_20_5";
}

export function isJava1212Family(version: GiveVersion): boolean {
  return version === "java_1_21_2" || version === "java_1_21_3" || version === "java_1_21_4";
}

function normalizeInt(value: unknown, fallback: number, min: number): number {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(min, Math.floor(num));
}

function normalizeNumber(value: unknown, fallback: number, min: number): number {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(min, num);
}

function cryptoId(): string {
  if ("crypto" in globalThis && "randomUUID" in crypto) return crypto.randomUUID();
  return String(Date.now());
}
