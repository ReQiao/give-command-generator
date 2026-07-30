/**
 * /setblock 指令构建器。
 *
 * 语法（1.20.5+）：
 *   setblock <x> <y> <z> <block>[blockstate]{nbt} [replace|destroy|keep]
 *
 * 方块实体 NBT 实测真值（semantic-probe 1.20.6 / 1.21.5，两版本一致）：
 *   - 命令方块：{Command:"...", auto:1b, TrackOutput:0b}
 *   - 容器（chest/barrel/hopper）：{Items:[{Slot:0b, id:"...", count:5, components:{...}}]}
 *   - 告示牌：{front_text:{messages:['...',…],color:"black",has_glowing_text:0b}, …}
 */

import { boolByte, namespaced, quote, type GiveVersion } from "../builder";
import { serializeContainerItem, type NbtItem } from "./nbt";

export type SetblockMode = "replace" | "destroy" | "keep";

export interface ContainerSlot {
  slot: number;
  item: NbtItem;
}

export interface SetblockCommandBlockOptions {
  command: string;
  /** 始终激活（循环/连锁命令方块常用）。 */
  auto?: boolean;
  trackOutput?: boolean;
}

export interface SetblockForm {
  version: GiveVersion;
  withSlash?: boolean;
  /** 坐标，支持 "0" / "~" / "~1" / "^1" 写法。 */
  x: string;
  y: string;
  z: string;
  block: string;
  /** 方块状态，形如 `facing=up,conditional=false`（不含方括号）。 */
  blockstate?: string;
  mode?: SetblockMode;
  /** 方块实体 NBT：三选一，或都留空。 */
  commandBlock?: SetblockCommandBlockOptions;
  containerItems?: ContainerSlot[];
  signLines?: [string, string, string, string];
}

export function buildSetblockCommand(form: SetblockForm): string {
  const block = namespaced(form.block);
  const blockstate = form.blockstate ? `[${form.blockstate}]` : "";
  const nbt = buildNbt(form);
  const mode = form.mode && form.mode !== "replace" ? ` ${form.mode}` : "";

  // NBT compound 紧贴方块标识符（无空格）：minecraft:command_block[facing=up]{Command:…}
  const cmd = `setblock ${form.x} ${form.y} ${form.z} ${block}${blockstate}${nbt}${mode}`;
  return form.withSlash ? `/${cmd}` : cmd;
}

function buildNbt(form: SetblockForm): string {
  if (form.commandBlock) return buildCommandBlockNbt(form.commandBlock);
  if (form.containerItems && form.containerItems.length > 0) return buildContainerNbt(form.containerItems);
  if (form.signLines) return buildSignNbt(form.signLines);
  return "";
}

function buildCommandBlockNbt(opts: SetblockCommandBlockOptions): string {
  // 命令方块内部存的命令不带前导斜杠。
  const parts: string[] = [`Command:${quote(opts.command.trim().replace(/^\//, ""))}`];
  if (opts.auto) parts.push("auto:1b");
  if (opts.trackOutput === false) parts.push("TrackOutput:0b");
  return `{${parts.join(",")}}`;
}

function buildContainerNbt(slots: ContainerSlot[]): string {
  return `{Items:[${slots.map(({ slot, item }) => serializeContainerItem(slot, item)).join(",")}]}`;
}

/** 告示牌 NBT（1.20+ front_text/back_text 格式，两版本一致）。四行定长，空行也要占位。 */
function buildSignNbt(lines: [string, string, string, string]): string {
  const messages = (texts: string[]) => texts.map((text) => quote(JSON.stringify({ text }))).join(",");
  const front = messages(lines);
  const back = messages(["", "", "", ""]);
  return (
    `{front_text:{messages:[${front}],color:"black",has_glowing_text:${boolByte(false)}},` +
    `back_text:{messages:[${back}],color:"black",has_glowing_text:${boolByte(false)}},` +
    `is_waxed:${boolByte(false)}}`
  );
}
