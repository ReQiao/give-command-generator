/**
 * /summon 指令构建器。
 *
 * 语法（1.20.5+）：
 *   summon <entity_type> [<x> <y> <z>] [{nbt_compound}]
 *
 * 实测真值（semantic-probe 1.20.6 / 1.21.5）：
 *   - CustomName：SNBT 字符串 '{"text":"..."}' 两版本均有效
 *   - 属性：1.20.5-1.21.4 用 Attributes[]/Name(generic.前缀)/Base；1.21.5+ 用 attributes[]/id/base
 *   - 效果：两版本均用 active_effects[]/id(string)/amplifier(byte)
 *   - 装备：1.20.5-1.21.4 用 HandItems[mainhand,offhand] / ArmorItems[feet,legs,chest,head]
 *           1.21.5+ 用 equipment{mainhand,offhand,head,chest,legs,feet}
 *   - Passengers：嵌套实体，id 字段（entity type，小写 "minecraft:chicken"）
 */

import { namespaced, quote, boolByte } from "../snbt";
import type { GiveVersion, RichLine } from "../types";
import {
  isModernNbtFamily,
  serializeCustomName,
  serializeAttributes,
  serializeEffects,
  serializeEquipment,
  type NbtAttribute,
  type NbtEffect,
  type NbtEquipment,
} from "./nbt";

// -----------------------------------------------------------------------
// 表单类型
// -----------------------------------------------------------------------

export interface SummonForm {
  version: GiveVersion;
  withSlash?: boolean;
  entityType: string;
  // 坐标（支持 "~" "~1" "0" 等，省略时不输出坐标）
  x?: string;
  y?: string;
  z?: string;
  // 通用 NBT 字段
  customName?: RichLine;
  noAI?: boolean;
  silent?: boolean;
  persistenceRequired?: boolean;
  invulnerable?: boolean;
  noGravity?: boolean;
  glowing?: boolean;
  tags?: string[];
  // 属性（如 max_health 等）
  attributes?: NbtAttribute[];
  // 状态效果
  effects?: NbtEffect[];
  // 装备
  equipment?: NbtEquipment;
  // 嵌套实体（骑乘）
  passengers?: SummonPassenger[];
  // 任意附加 NBT 字段（已序列化的 SNBT 片段，逗号分隔）
  extraNbt?: string;
}

export interface SummonPassenger {
  entityType: string;
  noAI?: boolean;
  silent?: boolean;
  customName?: RichLine;
  extraNbt?: string;
}

// -----------------------------------------------------------------------
// 主入口
// -----------------------------------------------------------------------

export function buildSummonCommand(form: SummonForm): string {
  const type = namespaced(form.entityType);
  const modern = isModernNbtFamily(form.version);

  const nbtParts = buildNbtParts(form, modern);
  const hasNbt = nbtParts.length > 0;

  let cmd = `summon ${type}`;

  const hasPos = form.x !== undefined && form.y !== undefined && form.z !== undefined;
  if (hasPos || hasNbt) {
    const x = form.x ?? "~";
    const y = form.y ?? "~";
    const z = form.z ?? "~";
    cmd += ` ${x} ${y} ${z}`;
  }

  if (hasNbt) {
    cmd += ` {${nbtParts.join(",")}}`;
  }

  return form.withSlash ? `/${cmd}` : cmd;
}

// -----------------------------------------------------------------------
// NBT 片段构建
// -----------------------------------------------------------------------

function buildNbtParts(form: SummonForm, modern: boolean): string[] {
  const parts: string[] = [];

  if (form.customName && form.customName.length > 0 && form.customName.flat().length > 0) {
    parts.push(`CustomName:${serializeCustomName(form.customName)}`);
  }
  if (form.noAI) parts.push(`NoAI:${boolByte(true)}`);
  if (form.silent) parts.push(`Silent:${boolByte(true)}`);
  if (form.persistenceRequired) parts.push(`PersistenceRequired:${boolByte(true)}`);
  if (form.invulnerable) parts.push(`Invulnerable:${boolByte(true)}`);
  if (form.noGravity) parts.push(`NoGravity:${boolByte(true)}`);
  if (form.glowing) parts.push(`Glowing:${boolByte(true)}`);

  if (form.tags && form.tags.length > 0) {
    const tagList = form.tags.map((t) => quote(t)).join(",");
    parts.push(`Tags:[${tagList}]`);
  }

  if (form.attributes && form.attributes.length > 0) {
    parts.push(serializeAttributes(form.attributes, modern));
  }

  if (form.effects && form.effects.length > 0) {
    parts.push(serializeEffects(form.effects));
  }

  if (form.equipment) {
    parts.push(...serializeEquipment(form.equipment, modern));
  }

  if (form.passengers && form.passengers.length > 0) {
    const passList = form.passengers.map((p) => buildPassengerNbt(p)).join(",");
    parts.push(`Passengers:[${passList}]`);
  }

  if (form.extraNbt) {
    parts.push(form.extraNbt.trim());
  }

  return parts;
}

function buildPassengerNbt(p: SummonPassenger): string {
  const inner: string[] = [`id:${quote(namespaced(p.entityType))}`];
  if (p.noAI) inner.push(`NoAI:${boolByte(true)}`);
  if (p.silent) inner.push(`Silent:${boolByte(true)}`);
  if (p.customName && p.customName.length > 0) {
    inner.push(`CustomName:${serializeCustomName(p.customName)}`);
  }
  if (p.extraNbt) inner.push(p.extraNbt.trim());
  return `{${inner.join(",")}}`;
}
