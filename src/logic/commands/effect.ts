/**
 * /effect 指令构建器。
 *
 * 语法（1.19.4+ 全版本一致，覆盖 1.20.5 ~ 26.x）：
 *   effect give <target> <effect> [<seconds>|infinite] [<amplifier>] [<hideParticles>]
 *   effect clear <target> [<effect>]
 *
 * mc-verifier 探针实测（1.20.6 / 1.21.5 均有效）：
 *   effect give @a minecraft:speed / …30 / …30 2 / …30 2 true / …infinite 1
 *   effect clear @a / effect clear @a minecraft:speed
 */

import { namespaced } from "../builder";

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

export function buildEffectGiveCommand(form: EffectGiveForm): string {
  const parts: string[] = [`effect give ${form.target} ${namespaced(form.effect)}`];

  // duration / amplifier / hideParticles 是位置参数：后面的要写，前面的就必须显式补出。
  const hasDuration = form.duration !== undefined;
  const hasAmplifier = form.amplifier !== undefined;
  const hasHide = form.hideParticles !== undefined;

  if (hasDuration || hasAmplifier || hasHide) {
    parts.push(form.duration !== undefined ? String(form.duration) : "30");
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
