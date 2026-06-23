/**
 * /effect 指令构建器。
 *
 * 语法（1.19.4+ 全版本一致，覆盖 1.20.6 ~ 1.21.5）：
 *   effect give <target> <effect> [<seconds>|infinite] [<amplifier>] [<hideParticles>]
 *   effect clear <target> [<effect>]
 *
 * 实测探针验证（1.20.6 / 1.21.5 均有效）：
 *   effect give @a minecraft:speed              → 有效（仅效果 id）
 *   effect give @a minecraft:speed 30           → 有效（+时长）
 *   effect give @a minecraft:speed 30 2         → 有效（+等级）
 *   effect give @a minecraft:speed 30 2 true    → 有效（+隐藏粒子）
 *   effect give @a minecraft:speed infinite 1   → 有效（无限时长）
 *   effect clear @a                             → 有效（清除全部）
 *   effect clear @a minecraft:speed             → 有效（清除指定）
 */

import { namespaced } from "../snbt";

// -----------------------------------------------------------------------
// 表单类型
// -----------------------------------------------------------------------

export interface EffectGiveForm {
  withSlash?: boolean;
  target: string;
  effect: string;
  /** 时长（秒）。传 "infinite" 表示无限。省略时服务器默认 30 秒。 */
  duration?: number | "infinite";
  /** 效果等级，0 = I，1 = II，依此类推。省略时服务器默认 0（I 级）。 */
  amplifier?: number;
  /** 隐藏粒子效果。省略时服务器默认 false（显示粒子）。 */
  hideParticles?: boolean;
}

export interface EffectClearForm {
  withSlash?: boolean;
  target: string;
  /** 省略时清除该目标所有效果。 */
  effect?: string;
}

// -----------------------------------------------------------------------
// 主入口
// -----------------------------------------------------------------------

export function buildEffectGiveCommand(form: EffectGiveForm): string {
  const effect = namespaced(form.effect);
  const parts: string[] = [`effect give ${form.target} ${effect}`];

  // duration、amplifier、hideParticles 是可选的位置参数，必须按顺序追加。
  // 只要后面有参数，前面的就必须显式写出。
  const hasDuration = form.duration !== undefined;
  const hasAmplifier = form.amplifier !== undefined;
  const hasHide = form.hideParticles !== undefined;

  if (hasDuration || hasAmplifier || hasHide) {
    const dur = form.duration !== undefined ? String(form.duration) : "30";
    parts.push(dur);
  }
  if (hasAmplifier || hasHide) {
    parts.push(String(form.amplifier ?? 0));
  }
  if (hasHide) {
    parts.push(form.hideParticles ? "true" : "false");
  }

  const cmd = parts.join(" ");
  return form.withSlash ? `/${cmd}` : cmd;
}

export function buildEffectClearCommand(form: EffectClearForm): string {
  const parts = [`effect clear ${form.target}`];
  if (form.effect) parts.push(namespaced(form.effect));
  const cmd = parts.join(" ");
  return form.withSlash ? `/${cmd}` : cmd;
}
