/**
 * /enchant 指令构建器。
 *
 * 语法（1.20.5+ 全版本一致）：enchant <targets> <enchantment> [<level>]
 *
 * 附魔与物品是否兼容、是否超过最高等级由服务器运行时裁决，不属于语法问题，
 * 这里只保证语法正确。
 */

import { namespaced } from "../builder";

export interface EnchantForm {
  withSlash?: boolean;
  targets: string;
  enchantment: string;
  /** 省略时默认 1。 */
  level?: number;
}

export function buildEnchantCommand(form: EnchantForm): string {
  const parts = [`enchant ${form.targets} ${namespaced(form.enchantment)}`];
  if (form.level !== undefined) parts.push(String(form.level));
  const cmd = parts.join(" ");
  return form.withSlash ? `/${cmd}` : cmd;
}
