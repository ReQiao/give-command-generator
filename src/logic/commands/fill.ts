/**
 * /fill 指令构建器。
 *
 * 语法（1.20.5+ 全版本一致）：
 *   fill <from x y z> <to x y z> <block>[blockstate]{nbt} [destroy|hollow|keep|outline|replace]
 *   fill <from> <to> <block> replace <filterBlock>[filterState]
 *
 * 方块参数与 /setblock 同源：blockstate 用 [..]，方块实体 NBT 用 {..}，两者紧贴方块 id。
 */

import { namespaced } from "../builder";

export type FillMode = "replace" | "destroy" | "hollow" | "keep" | "outline";

export type Coords = [string, string, string];

export interface BlockFilter {
  block: string;
  blockstate?: string;
}

export interface FillForm {
  withSlash?: boolean;
  from: Coords;
  to: Coords;
  block: string;
  blockstate?: string;
  /** 已序列化的方块实体 NBT 片段，如 `{Command:"say hi"}`。 */
  nbt?: string;
  mode?: FillMode;
  /** 指定后强制走 replace 模式，只替换该目标方块。 */
  replaceFilter?: BlockFilter;
}

/** 把 block + blockstate + nbt 拼成紧贴的方块参数。 */
export function blockSpec(block: string, blockstate?: string, nbt?: string): string {
  const state = blockstate ? `[${blockstate}]` : "";
  return `${namespaced(block)}${state}${nbt ? nbt.trim() : ""}`;
}

export function buildFillCommand(form: FillForm): string {
  let cmd = `fill ${form.from.join(" ")} ${form.to.join(" ")} ${blockSpec(form.block, form.blockstate, form.nbt)}`;

  if (form.replaceFilter) {
    cmd += ` replace ${blockSpec(form.replaceFilter.block, form.replaceFilter.blockstate)}`;
  } else if (form.mode && form.mode !== "replace") {
    cmd += ` ${form.mode}`;
  }

  return form.withSlash ? `/${cmd}` : cmd;
}
