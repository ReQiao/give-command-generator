/**
 * 实体与方块实体 NBT 序列化器（与 /give 的 item 组件格式无关）。
 *
 * 实测真值（semantic-probe 1.20.6 / 1.21.5）：
 *   - item-in-NBT：{id:"minecraft:stone", count:5, Slot:0b, components:{...}}
 *     count 小写，id 小写，Slot 大写，Count 大写旧键被静默丢弃
 *   - attributes：1.20.5-1.21.4 用 Attributes[]/Name/Base（generic.前缀）；1.21.5+ 用 attributes[]/id/base（无前缀）
 *   - active_effects：两版本均用 active_effects[]/id(string)，旧 ActiveEffects/Id(int) 被忽略
 *   - equipment（实体装备槽）：1.20.5-1.21.4 用 HandItems[]/ArmorItems[]；1.21.5+ 用 equipment{mainhand,...}
 *   - CustomName：SNBT 字符串两版本均有效；裸 JSON compound 仅 1.21.5 接受
 *   - 命令方块：Command(大写)/auto(小写)/TrackOutput(大写)
 */

import { namespaced, quote, snbtJsonString, boolByte, fmtNumber } from "../snbt";
import type { GiveVersion, TextComponent, RichLine } from "../types";

/** 版本属于 1.21.5+ 现代族（新属性格式 / equipment compound）。 */
export function isModernNbtFamily(version: GiveVersion): boolean {
  return (
    version === "java_1_21_5" ||
    version === "java_1_21_6" ||
    version === "java_1_21_9" ||
    version === "java_1_21_11_plus" ||
    version === "java_26_1" ||
    version === "java_26_2_plus"
  );
}

// -----------------------------------------------------------------------
// Item-in-NBT（chest Items[] / entity HandItems[] / entity equipment{}）
// -----------------------------------------------------------------------

export interface NbtItem {
  id: string;
  count?: number;
  /** 已序列化的 SNBT 值，例如 `'{"text":"x"}'` 或 `1b` */
  components?: Record<string, string>;
}

/**
 * 序列化一个 item-in-NBT（不含 Slot，由调用方决定是否加）。
 * 实测：count 小写 int，id 小写字符串，components 小写，键用完整命名空间。
 */
export function serializeItem(item: NbtItem): string {
  const parts: string[] = [`id:${quote(namespaced(item.id))}`, `count:${item.count ?? 1}`];
  const comps = item.components;
  if (comps && Object.keys(comps).length > 0) {
    const inner = Object.entries(comps)
      .map(([k, v]) => `${quote(namespaced(k))}:${v}`)
      .join(",");
    parts.push(`components:{${inner}}`);
  }
  return `{${parts.join(",")}}`;
}

/**
 * 序列化容器 slot（chest / barrel / hopper 等）。
 * Slot 大写 byte，置于首位。
 */
export function serializeContainerItem(slot: number, item: NbtItem): string {
  const base = serializeItem(item);
  // 在 { 后插入 Slot 字段
  return `{Slot:${slot}b,${base.slice(1)}`;
}

// -----------------------------------------------------------------------
// CustomName（文本组件）
// -----------------------------------------------------------------------

function textComponentToJson(line: TextComponent[]): unknown {
  if (line.length === 0) return { text: "" };
  if (line.length === 1) {
    const { text, ...rest } = line[0];
    return Object.keys(rest).length === 0 ? { text } : line[0];
  }
  return line;
}

/**
 * 序列化 CustomName 文本。
 * 所有版本：SNBT 字符串 '{"text":"..."}' 均有效。
 * 1.21.5+ 也接受裸 JSON，但为统一用 SNBT 字符串。
 */
export function serializeCustomName(line: RichLine): string {
  return snbtJsonString(textComponentToJson(line));
}

// -----------------------------------------------------------------------
// 属性（Attributes / attributes）
// -----------------------------------------------------------------------

export interface NbtAttribute {
  /** 属性 id，如 "max_health" / "minecraft:max_health" / "minecraft:generic.max_health" */
  id: string;
  base: number;
}

/**
 * 序列化属性列表。
 * 实测：
 *   early/legacy/mid：Attributes[]/Name(含 generic.前缀)/Base  — 旧格式
 *   modern：attributes[]/id(无前缀)/base                        — 新格式
 */
export function serializeAttributes(attrs: NbtAttribute[], modern: boolean): string {
  if (attrs.length === 0) return "";
  if (modern) {
    const entries = attrs.map(({ id, base }) => {
      const nid = normalizeAttrId(id, false);
      return `{id:${quote(nid)},base:${fmtNumber(base)}d}`;
    });
    return `attributes:[${entries.join(",")}]`;
  } else {
    const entries = attrs.map(({ id, base }) => {
      const name = normalizeAttrId(id, true);
      return `{Name:${quote(name)},Base:${fmtNumber(base)}d}`;
    });
    return `Attributes:[${entries.join(",")}]`;
  }
}

/**
 * 把各种输入格式归一化为目标格式的属性 id。
 * withGeneric=true → "minecraft:generic.max_health"
 * withGeneric=false → "minecraft:max_health"
 */
function normalizeAttrId(raw: string, withGeneric: boolean): string {
  // 拆出基础部分
  let name = raw.trim();
  // 去掉 minecraft: 前缀
  if (name.startsWith("minecraft:")) name = name.slice("minecraft:".length);
  // 去掉 generic. 前缀
  if (name.startsWith("generic.")) name = name.slice("generic.".length);
  if (withGeneric) return `minecraft:generic.${name}`;
  return `minecraft:${name}`;
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

/**
 * 序列化状态效果列表。
 * 实测：两版本均用 active_effects(小写)/id(string)/amplifier(byte)。
 * 旧 ActiveEffects/Id(int) 被静默忽略。
 */
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
// 实体装备槽（HandItems / equipment）
// -----------------------------------------------------------------------

export interface NbtEquipment {
  mainhand?: NbtItem;
  offhand?: NbtItem;
  head?: NbtItem;
  chest?: NbtItem;
  legs?: NbtItem;
  feet?: NbtItem;
}

/**
 * 序列化实体装备。
 * 实测：
 *   early/legacy/mid：HandItems:[mainhand, offhand], ArmorItems:[feet,legs,chest,head]
 *   modern：equipment:{mainhand:{...}, offhand:{...}, head:{...}, ...}
 */
export function serializeEquipment(eq: NbtEquipment, modern: boolean): string[] {
  const parts: string[] = [];
  if (modern) {
    const slots: Array<[keyof NbtEquipment, string]> = [
      ["mainhand", "mainhand"],
      ["offhand", "offhand"],
      ["head", "head"],
      ["chest", "chest"],
      ["legs", "legs"],
      ["feet", "feet"],
    ];
    const filled = slots.filter(([k]) => eq[k]);
    if (filled.length === 0) return [];
    const inner = filled.map(([k, name]) => `${name}:${serializeItem(eq[k]!)}`).join(",");
    parts.push(`equipment:{${inner}}`);
  } else {
    const mainhand = eq.mainhand ? serializeItem(eq.mainhand) : "{}";
    const offhand = eq.offhand ? serializeItem(eq.offhand) : "{}";
    parts.push(`HandItems:[${mainhand},${offhand}]`);

    const feet = eq.feet ? serializeItem(eq.feet) : "{}";
    const legs_ = eq.legs ? serializeItem(eq.legs) : "{}";
    const chest_ = eq.chest ? serializeItem(eq.chest) : "{}";
    const head = eq.head ? serializeItem(eq.head) : "{}";
    parts.push(`ArmorItems:[${feet},${legs_},${chest_},${head}]`);
  }
  return parts;
}
