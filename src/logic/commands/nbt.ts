/**
 * 实体与方块实体 NBT 序列化器（与 /give 的 item 组件格式无关）。
 *
 * 这是跨指令复用的核心：setblock 的容器物品、summon 的装备/手持物品都走同一套
 * item-in-NBT 结构，只需在此写一次。
 *
 * 实测真值（mc-verifier semantic-probe 1.20.6 / 1.21.5）：
 *   - item-in-NBT：{id:"minecraft:stone", count:5, Slot:0b, components:{...}}
 *     count 小写，id 小写，Slot 大写；Count 大写旧键被静默丢弃
 *   - attributes：1.20.5-1.21.4 用 Attributes[]/Name/Base（generic. 前缀）；1.21.5+ 用 attributes[]/id/base（无前缀）
 *   - active_effects：两版本均用 active_effects[]/id(string)；旧 ActiveEffects/Id(int) 被忽略
 *   - equipment：1.20.5-1.21.4 用 HandItems[]/ArmorItems[]；1.21.5+ 用 equipment{mainhand,...}
 *   - CustomName：SNBT 字符串两版本均有效；裸 JSON compound 仅 1.21.5+ 接受
 *   - 命令方块：Command(大写)/auto(小写)/TrackOutput(大写)
 */

import {
  boolByte,
  componentId,
  fmtNumber,
  isJava121LegacyFamily,
  namespaced,
  quote,
  richLineToSnbtString,
  type GiveVersion,
  type RichLine,
} from "../builder";

export { isModernNbtFamily } from "../builder";

// -----------------------------------------------------------------------
// Item-in-NBT（容器 Items[] / 实体 HandItems[] / 实体 equipment{}）
// -----------------------------------------------------------------------

export interface NbtEnchantment {
  id: string;
  level: number;
}

export interface NbtItem {
  id: string;
  count?: number;
  /** 附魔列表；序列化规则与 /give 完全一致（按版本自动切换 levels 包装），
   *  调用方（包括 AI 意图）不需要手写 enchantments 组件的原始 SNBT。 */
  enchantments?: NbtEnchantment[];
  /** 已序列化的 SNBT 值，例如 `'{"text":"x"}'` 或 `1b` */
  components?: Record<string, string>;
}

/** 附魔组件值（不含 key），1.20.5~1.21.1 需要 levels 包装，其余版本直接铺开。 */
function serializeEnchantmentsValue(enchants: NbtEnchantment[], version: GiveVersion): string {
  const entries = enchants
    .filter((e) => String(e.id ?? "").trim())
    .map(({ id, level }) => `${componentId(id)}:${level}`)
    .join(",");
  return isJava121LegacyFamily(version) ? `{levels:{${entries}}}` : `{${entries}}`;
}

/**
 * 序列化一个 item-in-NBT（不含 Slot，由调用方决定是否加）。
 * version 缺省时不处理 enchantments（容器物品目前不需要）。
 */
export function serializeItem(item: NbtItem, version?: GiveVersion): string {
  const parts: string[] = [`id:${quote(namespaced(item.id))}`, `count:${item.count ?? 1}`];
  const comps: Record<string, string> = { ...(item.components ?? {}) };
  if (version && item.enchantments && item.enchantments.length > 0) {
    comps.enchantments = serializeEnchantmentsValue(item.enchantments, version);
  }
  if (Object.keys(comps).length > 0) {
    const inner = Object.entries(comps)
      .map(([k, v]) => `${quote(namespaced(k))}:${v}`)
      .join(",");
    parts.push(`components:{${inner}}`);
  }
  return `{${parts.join(",")}}`;
}

/** 序列化容器 slot（chest / barrel / hopper 等）。Slot 大写 byte，置于首位。 */
export function serializeContainerItem(slot: number, item: NbtItem, version?: GiveVersion): string {
  return `{Slot:${slot}b,${serializeItem(item, version).slice(1)}`;
}

// -----------------------------------------------------------------------
// CustomName（文本组件）
// -----------------------------------------------------------------------

/**
 * 序列化 CustomName 文本。所有版本均用 SNBT 字符串写法（1.21.5+ 也接受裸 JSON，
 * 但统一走 SNBT 以免分叉）。文本内容复用 /give 的富文本序列化管线。
 */
export function serializeCustomName(line: RichLine, version: GiveVersion): string {
  return richLineToSnbtString(line, version);
}

// -----------------------------------------------------------------------
// 属性（Attributes / attributes）
// -----------------------------------------------------------------------

export interface NbtAttribute {
  /** 属性 id，可带或不带 minecraft: / generic. 前缀，序列化时按版本归一化。 */
  id: string;
  base: number;
}

/**
 * 把各种输入格式归一化为目标格式的属性 id。
 * withGeneric=true → "minecraft:generic.max_health"（1.20.5~1.21.4）
 * withGeneric=false → "minecraft:max_health"（1.21.5+）
 */
export function normalizeAttributeId(raw: string, withGeneric: boolean): string {
  let name = String(raw ?? "").trim();
  if (name.startsWith("minecraft:")) name = name.slice("minecraft:".length);
  if (name.startsWith("generic.")) name = name.slice("generic.".length);
  return withGeneric ? `minecraft:generic.${name}` : `minecraft:${name}`;
}

/** 序列化属性列表（modern 决定新旧键名，见文件头真值表）。 */
export function serializeAttributes(attrs: NbtAttribute[], modern: boolean): string {
  if (attrs.length === 0) return "";
  if (modern) {
    const entries = attrs.map(({ id, base }) => `{id:${quote(normalizeAttributeId(id, false))},base:${fmtNumber(base)}d}`);
    return `attributes:[${entries.join(",")}]`;
  }
  const entries = attrs.map(({ id, base }) => `{Name:${quote(normalizeAttributeId(id, true))},Base:${fmtNumber(base)}d}`);
  return `Attributes:[${entries.join(",")}]`;
}

// -----------------------------------------------------------------------
// 状态效果（active_effects）
// -----------------------------------------------------------------------

export interface NbtEffect {
  id: string;
  duration?: number;
  amplifier?: number;
  showParticles?: boolean;
}

/** 序列化状态效果列表。两版本均用 active_effects(小写)/id(string)/amplifier(byte)。 */
export function serializeEffects(effects: NbtEffect[]): string {
  if (effects.length === 0) return "";
  const entries = effects.map(({ id, duration = 200, amplifier = 0, showParticles = true }) => {
    const fields = [
      `id:${quote(namespaced(id))}`,
      `duration:${duration}`,
      `amplifier:${amplifier}b`,
      `show_particles:${boolByte(showParticles)}`,
    ];
    return `{${fields.join(",")}}`;
  });
  return `active_effects:[${entries.join(",")}]`;
}

// -----------------------------------------------------------------------
// 实体装备槽（HandItems / ArmorItems / equipment）
// -----------------------------------------------------------------------

export interface NbtEquipment {
  mainhand?: NbtItem;
  offhand?: NbtItem;
  head?: NbtItem;
  chest?: NbtItem;
  legs?: NbtItem;
  feet?: NbtItem;
}

const EQUIPMENT_SLOTS: Array<keyof NbtEquipment> = ["mainhand", "offhand", "head", "chest", "legs", "feet"];

/**
 * 序列化实体装备，返回 0~2 个 NBT 片段。
 *   1.20.5~1.21.4：HandItems:[mainhand,offhand] + ArmorItems:[feet,legs,chest,head]
 *   1.21.5+：      equipment:{mainhand:{...},...}
 * version 传入时装备物品上的 enchantments 会按版本正确序列化（见 serializeItem）。
 */
export function serializeEquipment(eq: NbtEquipment, modern: boolean, version?: GiveVersion): string[] {
  if (modern) {
    const filled = EQUIPMENT_SLOTS.filter((slot) => eq[slot]);
    if (filled.length === 0) return [];
    const inner = filled.map((slot) => `${slot}:${serializeItem(eq[slot]!, version)}`).join(",");
    return [`equipment:{${inner}}`];
  }
  // 旧格式是定长数组，空槽位必须占位 {}；整组都空时干脆不输出该键。
  const slot = (key: keyof NbtEquipment) => (eq[key] ? serializeItem(eq[key]!, version) : "{}");
  const parts: string[] = [];
  if (eq.mainhand || eq.offhand) {
    parts.push(`HandItems:[${slot("mainhand")},${slot("offhand")}]`);
  }
  if (eq.feet || eq.legs || eq.chest || eq.head) {
    parts.push(`ArmorItems:[${slot("feet")},${slot("legs")},${slot("chest")},${slot("head")}]`);
  }
  return parts;
}
