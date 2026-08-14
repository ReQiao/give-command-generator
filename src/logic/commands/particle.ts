/**
 * /particle 指令构建器。
 *
 * 语法（全版本一致）：
 *   particle <name> [<pos>] [<delta> <speed> <count> [force|normal] [<viewers>]]
 *
 * 参数化粒子的附加数据写在粒子 id 后面、紧跟无空格，例如：
 *   minecraft:dust{color:[1.0,0.0,0.0],scale:1.0}                  — 红色尘埃，1.20.5+
 *   minecraft:dust_color_transition{from_color:[...],to_color:[...],scale:1.0}
 *   minecraft:block{block_state:{Name:"minecraft:stone"}}          — 方块碎裂粒子
 *   minecraft:item{item:{id:"minecraft:diamond",count:1}}          — 物品图标粒子
 * 这类粒子把完整 id（含附加数据）传进 name 字段即可，构建器只在花括号前的部分补
 * minecraft: 前缀，不会破坏花括号内的内容。
 */

import { fmtNumber, namespaced } from "../builder";

export interface ParticleForm {
  withSlash?: boolean;
  /** 粒子 id，可携带参数化附加数据（见文件头示例）。 */
  name: string;
  x?: string;
  y?: string;
  z?: string;
  /** 生成范围（每个方向的随机偏移半径）。 */
  dx?: number;
  dy?: number;
  dz?: number;
  /** 粒子速度/运动参数，含义因粒子类型而异。 */
  speed?: number;
  count?: number;
  /** force：无视客户端粒子设置和距离限制都显示；normal：按客户端设置和默认距离。 */
  mode?: "force" | "normal";
  /** 只让指定玩家看到，省略则视距内所有玩家可见。 */
  viewers?: string;
}

/** 只给花括号前的粒子 id 部分补 minecraft: 前缀，避免误伤附加数据里的冒号。 */
function namespacedParticleId(raw: string): string {
  const text = String(raw ?? "").trim();
  const braceIndex = text.indexOf("{");
  if (braceIndex === -1) return namespaced(text);
  return `${namespaced(text.slice(0, braceIndex))}${text.slice(braceIndex)}`;
}

export function buildParticleCommand(form: ParticleForm): string {
  const parts: string[] = [`particle ${namespacedParticleId(form.name)}`];

  const hasPos = form.x !== undefined && form.y !== undefined && form.z !== undefined;
  const hasExtra =
    form.dx !== undefined ||
    form.dy !== undefined ||
    form.dz !== undefined ||
    form.speed !== undefined ||
    form.count !== undefined ||
    form.mode !== undefined ||
    form.viewers !== undefined;

  if (hasPos || hasExtra) {
    parts.push(form.x ?? "~", form.y ?? "~", form.z ?? "~");
  }
  if (hasExtra) {
    parts.push(fmtNumber(form.dx ?? 0), fmtNumber(form.dy ?? 0), fmtNumber(form.dz ?? 0));
    parts.push(fmtNumber(form.speed ?? 1));
    parts.push(String(form.count ?? 1));
    if (form.mode || form.viewers) parts.push(form.mode ?? "normal");
    if (form.viewers) parts.push(form.viewers);
  }

  const cmd = parts.join(" ");
  return form.withSlash ? `/${cmd}` : cmd;
}
