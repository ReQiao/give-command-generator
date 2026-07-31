/**
 * /summon 指令构建器。
 *
 * 语法（1.20.5+）：
 *   summon <entity_type> [<x> <y> <z>] [{nbt_compound}]
 *
 * 版本敏感点全部下沉到 commands/nbt.ts（属性 / 装备 / CustomName），
 * 这里只负责组装。真值表见 nbt.ts 文件头。
 */

import { boolByte, fmtNumber, isModernNbtFamily, namespaced, quote, type GiveVersion, type RichLine } from "../builder";
import {
  serializeAttributes,
  serializeCustomName,
  serializeEffects,
  serializeEquipment,
  type NbtAttribute,
  type NbtEffect,
  type NbtEquipment,
} from "./nbt";

export interface SummonPassenger {
  entityType: string;
  noAI?: boolean;
  silent?: boolean;
  customName?: RichLine;
  /** 已序列化的附加 SNBT 片段（逗号分隔，不含外层花括号）。 */
  extraNbt?: string;
}

export interface SummonForm {
  version: GiveVersion;
  withSlash?: boolean;
  entityType: string;
  /** 坐标，支持 "0" / "~" / "~1" / "^1"；三者需同时提供，否则视为不指定。 */
  x?: string;
  y?: string;
  z?: string;
  customName?: RichLine;
  noAI?: boolean;
  silent?: boolean;
  persistenceRequired?: boolean;
  invulnerable?: boolean;
  noGravity?: boolean;
  glowing?: boolean;
  /** 出生朝向 [yaw, pitch]（度）。只影响朝向，不影响移动方向。 */
  rotation?: [number, number];
  /**
   * 当前生命值。改 max_health 属性只会改「上限」，不改「当前值」——
   * 不配这个字段，生物仍按旧上限（通常 20）的血量生成，看起来像没生效。
   * 想要生物一出生就满新血量，必须把这个设成和 max_health 属性相同的值。
   */
  health?: number;
  tags?: string[];
  attributes?: NbtAttribute[];
  effects?: NbtEffect[];
  equipment?: NbtEquipment;
  passengers?: SummonPassenger[];
  /** 已序列化的附加 SNBT 片段（逗号分隔，不含外层花括号）。 */
  extraNbt?: string;
}

export function buildSummonCommand(form: SummonForm): string {
  const modern = isModernNbtFamily(form.version);
  const nbtParts = buildNbtParts(form, modern);
  const hasNbt = nbtParts.length > 0;
  const hasPos = form.x !== undefined && form.y !== undefined && form.z !== undefined;

  let cmd = `summon ${namespaced(form.entityType)}`;
  // NBT 是第 5 个位置参数：要带 NBT 就必须先补出坐标。
  if (hasPos || hasNbt) {
    cmd += ` ${form.x ?? "~"} ${form.y ?? "~"} ${form.z ?? "~"}`;
  }
  if (hasNbt) {
    cmd += ` {${nbtParts.join(",")}}`;
  }
  return form.withSlash ? `/${cmd}` : cmd;
}

function buildNbtParts(form: SummonForm, modern: boolean): string[] {
  const parts: string[] = [];

  if (form.customName && form.customName.length > 0) {
    parts.push(`CustomName:${serializeCustomName(form.customName, form.version)}`);
  }
  if (form.noAI) parts.push(`NoAI:${boolByte(true)}`);
  if (form.silent) parts.push(`Silent:${boolByte(true)}`);
  if (form.persistenceRequired) parts.push(`PersistenceRequired:${boolByte(true)}`);
  if (form.invulnerable) parts.push(`Invulnerable:${boolByte(true)}`);
  if (form.noGravity) parts.push(`NoGravity:${boolByte(true)}`);
  if (form.glowing) parts.push(`Glowing:${boolByte(true)}`);
  if (form.rotation) parts.push(`Rotation:[${fmtNumber(form.rotation[0])}f,${fmtNumber(form.rotation[1])}f]`);
  if (form.health !== undefined) parts.push(`Health:${fmtNumber(form.health)}f`);

  if (form.tags && form.tags.length > 0) {
    parts.push(`Tags:[${form.tags.map((t) => quote(t)).join(",")}]`);
  }
  if (form.attributes && form.attributes.length > 0) {
    parts.push(serializeAttributes(form.attributes, modern));
  }
  if (form.effects && form.effects.length > 0) {
    parts.push(serializeEffects(form.effects));
  }
  if (form.equipment) {
    parts.push(...serializeEquipment(form.equipment, modern, form.version));
  }
  if (form.passengers && form.passengers.length > 0) {
    parts.push(`Passengers:[${form.passengers.map((p) => buildPassengerNbt(p, form.version)).join(",")}]`);
  }
  if (form.extraNbt && form.extraNbt.trim()) {
    parts.push(form.extraNbt.trim());
  }

  return parts;
}

function buildPassengerNbt(p: SummonPassenger, version: GiveVersion): string {
  const inner: string[] = [`id:${quote(namespaced(p.entityType))}`];
  if (p.noAI) inner.push(`NoAI:${boolByte(true)}`);
  if (p.silent) inner.push(`Silent:${boolByte(true)}`);
  if (p.customName && p.customName.length > 0) {
    inner.push(`CustomName:${serializeCustomName(p.customName, version)}`);
  }
  if (p.extraNbt && p.extraNbt.trim()) inner.push(p.extraNbt.trim());
  return `{${inner.join(",")}}`;
}
