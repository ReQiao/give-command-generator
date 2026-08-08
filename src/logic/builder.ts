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

// ---------------- 文本组件模型 ----------------
// 样式字段：可作用于任意内容类型（文本/翻译/object/…）。
export type ClickAction =
  | "open_url"
  | "run_command"
  | "suggest_command"
  | "copy_to_clipboard"
  | "change_page"
  | "show_dialog";

export interface ClickEvent {
  action: ClickAction;
  value?: string; // url / command / 剪贴板文本 / 页码 / dialog id
}

export type HoverAction = "show_text" | "show_item" | "show_entity";

export interface HoverEvent {
  action: HoverAction;
  text?: RichLine; // show_text
  itemId?: string; // show_item
  itemCount?: number;
  entityType?: string; // show_entity
  entityUuid?: string;
  entityName?: RichLine;
}

export interface TextStyle {
  bold?: boolean;
  italic?: boolean;
  underlined?: boolean;
  strikethrough?: boolean;
  obfuscated?: boolean;
  color?: string;
  font?: string;
  shadow_color?: number | number[];
  insertion?: string;
  click_event?: ClickEvent;
  hover_event?: HoverEvent;
}

// 内容类型（可辨识联合）：默认 text，兼容旧模板（无 type 字段视作 text）。
export interface TextComponent extends TextStyle {
  type?: "text";
  text: string;
}

export interface TranslatableComponent extends TextStyle {
  type: "translatable";
  translate: string;
  fallback?: string;
  with?: RichComponent[];
}

export interface ObjectSpriteComponent extends TextStyle {
  type: "object";
  object?: "atlas";
  atlas?: string;
  sprite: string;
}

export interface ObjectPlayerComponent extends TextStyle {
  type: "object";
  object: "player";
  player: string;
  hat?: boolean;
}

export interface KeybindComponent extends TextStyle {
  type: "keybind";
  keybind: string;
}

export interface SelectorComponent extends TextStyle {
  type: "selector";
  selector: string;
  separator?: RichComponent;
}

export interface ScoreComponent extends TextStyle {
  type: "score";
  score: { name: string; objective: string };
}

export interface NbtComponent extends TextStyle {
  type: "nbt";
  nbt: string;
  source: "block" | "entity" | "storage";
  block?: string;
  entity?: string;
  storage?: string;
  interpret?: boolean;
  separator?: RichComponent;
}

export type RichComponent =
  | TextComponent
  | TranslatableComponent
  | ObjectSpriteComponent
  | ObjectPlayerComponent
  | KeybindComponent
  | SelectorComponent
  | ScoreComponent
  | NbtComponent;

export type RichLine = RichComponent[];

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

export function buildGiveCommand(form: GiveForm, warnings: string[] = []): string {
  if (form.version === "bedrock") {
    return buildBedrock(form);
  }
  if (isJava121LegacyFamily(form.version)) {
    return buildJava121Legacy(form, warnings);
  }
  if (isJava1205Family(form.version)) {
    return buildModernFamily(form, JAVA_1_20_5_PROFILE, warnings);
  }
  if (isJava1212Family(form.version)) {
    return buildModernFamily(form, JAVA_1_21_2_PROFILE, warnings);
  }

  return buildModernFamily(form, MODERN_PROFILE, warnings);
}

function buildModernFamily(form: GiveForm, profile: ModernProfile, warnings: string[] = []): string {
  const parts: string[] = [];
  const add = (name: string, value: string) => parts.push(`${name}=${value}`);
  const tp = resolveTextProfile(form.version);

  if (form.displayName.length) {
    add("custom_name", serializeText(form.displayName[0] ?? [], profile.textAsSnbtString, tp, warnings));
  }
  if (form.itemName.length) {
    add("item_name", serializeText(form.itemName[0] ?? [], profile.textAsSnbtString, tp, warnings));
  }
  if (form.lore.length) {
    if (profile.textAsSnbtString) {
      add("lore", `[${form.lore.map((line) => serializeText(line, true, tp, warnings)).join(",")}]`);
    } else {
      add("lore", `[${form.lore.map((line) => JSON.stringify(jsonRichLine(line, tp, warnings))).join(",")}]`);
    }
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

function buildJava121Legacy(form: GiveForm, warnings: string[] = []): string {
  const parts: string[] = [];
  const add = (name: string, value: string) => parts.push(`${name}=${value}`);
  const tp = resolveTextProfile(form.version);

  if (form.displayName.length) add("custom_name", serializeText(form.displayName[0] ?? [], true, tp, warnings));
  if (form.itemName.length) add("item_name", serializeText(form.itemName[0] ?? [], true, tp, warnings));
  if (form.lore.length) add("lore", `[${form.lore.map((line) => serializeText(line, true, tp, warnings)).join(",")}]`);

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

// ---------------- 文本组件序列化 ----------------
// 版本敏感能力（按 Java 版本先后判定，而非按粗粒度 item 组件 profile）：
//   object 组件      -> 1.21.9+
//   click/hover 新式 -> 1.21.5+（否则 camelCase 旧式）
//   shadow_color 数组 -> 1.21.4+（否则打包整数）
const JAVA_VERSION_ORDER: GiveVersion[] = [
  "java_1_20_5",
  "java_1_21",
  "java_1_21_1",
  "java_1_21_2",
  "java_1_21_3",
  "java_1_21_4",
  "java_1_21_5",
  "java_1_21_6",
  "java_1_21_9",
  "java_1_21_11_plus",
  "java_26_1",
  "java_26_2_plus",
];

function versionAtLeast(version: GiveVersion, min: GiveVersion): boolean {
  const iv = JAVA_VERSION_ORDER.indexOf(version);
  const im = JAVA_VERSION_ORDER.indexOf(min);
  return iv >= 0 && im >= 0 && iv >= im;
}

export interface TextProfile {
  supportsObjectComponent: boolean;
  eventFormatModern: boolean;
  supportsShadowArray: boolean;
}

export function resolveTextProfile(version: GiveVersion): TextProfile {
  return {
    supportsObjectComponent: versionAtLeast(version, "java_1_21_9"),
    eventFormatModern: versionAtLeast(version, "java_1_21_5"),
    supportsShadowArray: versionAtLeast(version, "java_1_21_4"),
  };
}

function normalizeShadow(shadow: number | number[], tp: TextProfile): number | number[] {
  if (Array.isArray(shadow)) {
    if (tp.supportsShadowArray) return shadow;
    // [r,g,b,a]（0~1 浮点）-> 打包 ARGB 整数（旧版本回退）
    const [r = 0, g = 0, b = 0, a = 1] = shadow;
    const R = Math.round(r * 255) & 0xff;
    const G = Math.round(g * 255) & 0xff;
    const B = Math.round(b * 255) & 0xff;
    const A = Math.round(a * 255) & 0xff;
    const value = (A << 24) | (R << 16) | (G << 8) | B;
    return value >= 2 ** 31 ? value - 2 ** 32 : value;
  }
  return shadow;
}

function shapeClickEvent(ev: ClickEvent, tp: TextProfile): { key: string; value: Record<string, unknown> } | null {
  if (!ev || !ev.action) return null;
  const val = ev.value ?? "";
  if (tp.eventFormatModern) {
    const out: Record<string, unknown> = { action: ev.action };
    switch (ev.action) {
      case "open_url": out.url = val; break;
      case "run_command": out.command = val; break;
      case "suggest_command": out.command = val; break;
      case "copy_to_clipboard": out.value = val; break;
      case "change_page": out.page = normalizeInt(val, 1, 1); break;
      case "show_dialog": out.dialog = val; break;
    }
    return { key: "click_event", value: out };
  }
  // 旧式 clickEvent{action,value}（show_dialog 为 1.21.6+，旧版不支持则丢弃）
  if (ev.action === "show_dialog") return null;
  const value = ev.action === "change_page" ? String(normalizeInt(val, 1, 1)) : val;
  return { key: "clickEvent", value: { action: ev.action, value } };
}

function shapeHoverEvent(
  ev: HoverEvent,
  tp: TextProfile,
  warnings: string[],
): { key: string; value: Record<string, unknown> } | null {
  if (!ev || !ev.action) return null;
  if (tp.eventFormatModern) {
    const out: Record<string, unknown> = { action: ev.action };
    if (ev.action === "show_text") {
      out.value = ev.text ? jsonRichLine(ev.text, tp, warnings) : "";
    } else if (ev.action === "show_item") {
      out.id = namespaced(ev.itemId || "stone");
      if (ev.itemCount !== undefined) out.count = ev.itemCount;
    } else if (ev.action === "show_entity") {
      out.id = namespaced(ev.entityType || "pig");
      if (ev.entityUuid) out.uuid = ev.entityUuid;
      if (ev.entityName) out.name = jsonRichLine(ev.entityName, tp, warnings);
    }
    return { key: "hover_event", value: out };
  }
  // 旧式 hoverEvent{action,contents:{...}}
  const out: Record<string, unknown> = { action: ev.action };
  if (ev.action === "show_text") {
    out.contents = ev.text ? jsonRichLine(ev.text, tp, warnings) : "";
  } else if (ev.action === "show_item") {
    const contents: Record<string, unknown> = { id: namespaced(ev.itemId || "stone") };
    if (ev.itemCount !== undefined) contents.count = ev.itemCount;
    out.contents = contents;
  } else if (ev.action === "show_entity") {
    const contents: Record<string, unknown> = { type: namespaced(ev.entityType || "pig") };
    if (ev.entityUuid) contents.id = ev.entityUuid;
    if (ev.entityName) contents.name = jsonRichLine(ev.entityName, tp, warnings);
    out.contents = contents;
  }
  return { key: "hoverEvent", value: out };
}

function applyStyle(out: Record<string, unknown>, style: TextStyle, tp: TextProfile, warnings: string[]): void {
  if (style.bold !== undefined) out.bold = style.bold;
  if (style.italic !== undefined) out.italic = style.italic;
  if (style.underlined !== undefined) out.underlined = style.underlined;
  if (style.strikethrough !== undefined) out.strikethrough = style.strikethrough;
  if (style.obfuscated !== undefined) out.obfuscated = style.obfuscated;
  if (style.color !== undefined) out.color = style.color;
  if (style.font !== undefined) out.font = style.font;
  if (style.shadow_color !== undefined) out.shadow_color = normalizeShadow(style.shadow_color, tp);
  if (style.insertion !== undefined) out.insertion = style.insertion;
  if (style.click_event) {
    const ce = shapeClickEvent(style.click_event, tp);
    if (ce) out[ce.key] = ce.value;
  }
  if (style.hover_event) {
    const he = shapeHoverEvent(style.hover_event, tp, warnings);
    if (he) out[he.key] = he.value;
  }
}

// 把一个运行整形为纯 JSON 对象（键名精确、按版本门控）。不合法/不支持返回 null（剥离）。
function componentToJson(run: RichComponent, tp: TextProfile, warnings: string[]): Record<string, unknown> | null {
  if (!run || typeof run !== "object") return null;
  const anyRun = run as unknown as Record<string, unknown>;
  const type = (anyRun.type as string) ?? "text";
  const out: Record<string, unknown> = {};

  switch (type) {
    case "translatable": {
      const r = run as TranslatableComponent;
      out.type = "translatable";
      out.translate = r.translate ?? "";
      if (r.fallback !== undefined) out.fallback = r.fallback;
      if (Array.isArray(r.with) && r.with.length) {
        out.with = r.with.map((c) => componentToJson(c, tp, warnings)).filter((v) => v !== null);
      }
      break;
    }
    case "object": {
      if (!tp.supportsObjectComponent) {
        warnings.push("内嵌图标/头像（object 组件）需要 Java 1.21.9+，已忽略");
        return null;
      }
      out.type = "object";
      if ((anyRun.object as string) === "player") {
        const r = run as ObjectPlayerComponent;
        out.object = "player";
        out.player = r.player ?? "";
        if (r.hat !== undefined) out.hat = r.hat;
      } else {
        const r = run as ObjectSpriteComponent;
        out.object = "atlas";
        out.atlas = r.atlas || "minecraft:blocks";
        out.sprite = r.sprite ?? "";
      }
      break;
    }
    case "keybind": {
      out.keybind = (run as KeybindComponent).keybind ?? "";
      break;
    }
    case "selector": {
      const r = run as SelectorComponent;
      out.selector = r.selector ?? "";
      if (r.separator) {
        const sep = componentToJson(r.separator, tp, warnings);
        if (sep) out.separator = sep;
      }
      break;
    }
    case "score": {
      const r = run as ScoreComponent;
      out.score = { name: r.score?.name ?? "", objective: r.score?.objective ?? "" };
      break;
    }
    case "nbt": {
      const r = run as NbtComponent;
      out.nbt = r.nbt ?? "";
      out.source = r.source ?? "block";
      if (r.source === "block" && r.block) out.block = r.block;
      if (r.source === "entity" && r.entity) out.entity = r.entity;
      if (r.source === "storage" && r.storage) out.storage = namespaced(r.storage);
      if (r.interpret !== undefined) out.interpret = r.interpret;
      if (r.separator) {
        const sep = componentToJson(r.separator, tp, warnings);
        if (sep) out.separator = sep;
      }
      break;
    }
    default: {
      // text（含无 type 的旧模板）
      out.text = String(anyRun.text ?? "");
    }
  }

  applyStyle(out, run as TextStyle, tp, warnings);
  return out;
}

function jsonRichLine(line: RichLine, tp: TextProfile, warnings: string[]): unknown[] {
  if (!Array.isArray(line)) return [];
  return line.map((run) => componentToJson(run, tp, warnings)).filter((v) => v !== null);
}

// 序列化单行文本组件：asSnbt 时包成单引号 SNBT 字符串（early/legacy/mid 族），否则裸 JSON（modern 族）。
function serializeText(line: RichLine, asSnbt: boolean, tp: TextProfile, warnings: string[]): string {
  const json = JSON.stringify(jsonRichLine(line, tp, warnings));
  if (!asSnbt) return json;
  return snbtJsonString(json);
}

/** 把已序列化的 JSON 文本包成 SNBT 单引号字符串（实体/方块实体 NBT 里的文本字段写法）。 */
export function snbtJsonString(json: string): string {
  return `'${json.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}

/**
 * 把一行富文本序列化为 SNBT 单引号字符串，供实体 CustomName / 方块实体文本使用。
 * 与 /give 的文本组件同源（同一套 jsonRichLine + 版本档案），保证两处写法一致。
 */
export function richLineToSnbtString(line: RichLine, version: GiveVersion, warnings: string[] = []): string {
  return serializeText(line, true, resolveTextProfile(version), warnings);
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

/**
 * 1.21.5+ 现代实体 NBT 族：属性用 attributes[]/id/base（无 generic. 前缀）、
 * 装备用 equipment{} 而非 HandItems[]/ArmorItems[]。基岩版不属于该族。
 */
export function isModernNbtFamily(version: GiveVersion): boolean {
  return versionAtLeast(version, "java_1_21_5");
}

export function getModernProfile(version: GiveVersion): ModernProfile {
  if (version === "bedrock" || isJava121LegacyFamily(version)) {
    return { textAsSnbtString: false, adventurePredicateWrapper: false, supportsTooltipDisplay: false, supportsConsumable: false, supportsGlider: false, supportsDeathProtection: false, supportsAttributeModifiers: false };
  }
  if (isJava1205Family(version)) {
    return JAVA_1_20_5_PROFILE;
  }
  if (isJava1212Family(version)) {
    return JAVA_1_21_2_PROFILE;
  }
  return MODERN_PROFILE;
}

/**
 * 把存档 level.dat 读到的原始版本字符串（如 "1.21.7"、"26.2"）映射到 GiveVersion 分档，
 * 供"识别到的存档版本和当前选择不一致"这类提示用（部署面板）。
 *
 * 边界要和 data/catalog.ts 的 VERSIONS 表（下拉框展示的"Java 1.21.6~1.21.8"之类
 * 范围文案）保持一致——两边各自维护而不是互相解析对方格式，是因为一个是给人看的
 * 文案、一个是给代码比较用的数字边界，写重复了但比互相解析更不容易出错，改版本
 * 分档时两边都要同步改。识别不出来（低于支持范围、格式不对）返回 null，
 * 调用方应该当成"识别不到，不打扰用户"处理，不强行覆盖用户已经选的版本。
 */
export function detectGiveVersionFromRaw(raw: string): GiveVersion | null {
  const parts = raw.trim().split(".").map((p) => Number.parseInt(p, 10));
  if (parts.length === 0 || parts.some((n) => Number.isNaN(n))) return null;

  // 新计年法（26.x 起）没有前导 "1."，直接按 major.minor 比较。
  if (parts[0] >= 2) {
    const [major, minor = 0] = parts;
    if (major === 26 && minor === 1) return "java_26_1";
    if (major >= 26 && minor >= 2) return "java_26_2_plus";
    if (major > 26) return "java_26_2_plus"; // 更新的年份先沿用最新分档
    return null;
  }

  // 老计年法：1.x.y
  if (parts[0] !== 1) return null;
  const minor = parts[1];
  const patch = parts[2] ?? 0;
  if (minor === 20) return patch >= 5 ? "java_1_20_5" : null;
  if (minor !== 21) return null;
  if (patch === 0) return "java_1_21";
  if (patch === 1) return "java_1_21_1";
  if (patch === 2) return "java_1_21_2";
  if (patch === 3) return "java_1_21_3";
  if (patch === 4) return "java_1_21_4";
  if (patch === 5) return "java_1_21_5";
  if (patch >= 6 && patch <= 8) return "java_1_21_6";
  if (patch >= 9 && patch <= 10) return "java_1_21_9";
  return "java_1_21_11_plus"; // patch >= 11
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
