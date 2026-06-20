/**
 * /setblock 指令构建器。
 *
 * 语法（1.20.5+）：
 *   setblock <x> <y> <z> <block>[blockstate]{nbt} [replace|destroy|keep]
 *
 * 方块实体 NBT 实测真值（semantic-probe 1.20.6 / 1.21.5，两版本一致）：
 *   - 命令方块：{Command:"...", auto:1b, TrackOutput:0b}
 *   - 容器（chest/barrel/hopper）：{Items:[{Slot:0b, id:"...", count:5, components:{...}}]}
 *   - 告示牌：{front_text:{messages:['...','...','...','...'],color:"red",has_glowing_text:0b}, ...}
 */

import { namespaced, quote, boolByte } from "../snbt";
import type { GiveVersion } from "../types";
import { serializeContainerItem, type NbtItem } from "./nbt";

export type SetblockMode = "replace" | "destroy" | "keep";

// -----------------------------------------------------------------------
// 表单类型
// -----------------------------------------------------------------------

export interface ContainerSlot {
  slot: number;
  item: NbtItem;
}

export interface SetblockCommandBlockOptions {
  command: string;
  auto?: boolean;
  trackOutput?: boolean;
  conditional?: boolean;
}

export interface SignLine {
  text: string;
}

export interface SetblockForm {
  version: GiveVersion;
  withSlash?: boolean;
  // 坐标（支持 "~" "~1" "0" 等）
  x: string;
  y: string;
  z: string;
  block: string;
  blockstate?: string;
  mode?: SetblockMode;
  // 方块实体 NBT（三选一，或留空）
  commandBlock?: SetblockCommandBlockOptions;
  containerItems?: ContainerSlot[];
  signLines?: [string, string, string, string];
}

// -----------------------------------------------------------------------
// 主入口
// -----------------------------------------------------------------------

export function buildSetblockCommand(form: SetblockForm): string {
  const block = namespaced(form.block);
  const blockstate = form.blockstate ? `[${form.blockstate}]` : "";
  const nbt = buildNbt(form);
  const mode = form.mode && form.mode !== "replace" ? ` ${form.mode}` : "";

  const pos = `${form.x} ${form.y} ${form.z}`;
  // NBT compound 紧贴方块标识符（无空格），如 minecraft:command_block[facing=up]{Command:...}
  const blockSpec = `${block}${blockstate}${nbt}`;
  const cmd = `setblock ${pos} ${blockSpec}${mode}`.trim();
  return form.withSlash ? `/${cmd}` : cmd;
}

// -----------------------------------------------------------------------
// NBT 构建
// -----------------------------------------------------------------------

function buildNbt(form: SetblockForm): string {
  if (form.commandBlock) return buildCommandBlockNbt(form.commandBlock);
  if (form.containerItems && form.containerItems.length > 0) return buildContainerNbt(form.containerItems);
  if (form.signLines) return buildSignNbt(form.signLines);
  return "";
}

function buildCommandBlockNbt(opts: SetblockCommandBlockOptions): string {
  const parts: string[] = [`Command:${quote(opts.command)}`];
  if (opts.auto) parts.push("auto:1b");
  if (opts.trackOutput === false) parts.push("TrackOutput:0b");
  return `{${parts.join(",")}}`;
}

function buildContainerNbt(slots: ContainerSlot[]): string {
  const items = slots.map(({ slot, item }) => serializeContainerItem(slot, item)).join(",");
  return `{Items:[${items}]}`;
}

/** 告示牌 NBT（1.20+ front_text/back_text 格式，两版本一致）。 */
function buildSignNbt(lines: [string, string, string, string]): string {
  const msgs = lines.map((text) => quote(JSON.stringify({ text }))).join(",");
  return `{front_text:{messages:[${msgs}],color:"black",has_glowing_text:${boolByte(false)}},back_text:{messages:['""','""','""','""'],color:"black",has_glowing_text:${boolByte(false)}},is_waxed:${boolByte(false)}}`;
}
