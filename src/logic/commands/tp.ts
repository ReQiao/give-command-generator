/**
 * /tp（/teleport）指令构建器。
 *
 * 语法（全版本一致，覆盖 1.20.5 ~ 26.x）：
 *   tp <targets> <x> <y> <z>                          — 传送到坐标
 *   tp <targets> <x> <y> <z> <yRot> <xRot>            — 传送到坐标并设定朝向
 *   tp <targets> <x> <y> <z> facing <fx> <fy> <fz>    — 传送到坐标并面朝某点
 *   tp <targets> <destination>                        — 传送到实体/玩家
 *
 * 坐标支持绝对（0）、相对（~、~1）、本地（^、^1）写法。
 */

/** 传送到坐标（可选朝向）。 */
export interface TpCoordsForm {
  withSlash?: boolean;
  /** 是否用 teleport 别名（默认 tp）。 */
  useTeleportAlias?: boolean;
  targets: string;
  x: string;
  y: string;
  z: string;
  /** 偏航角 yaw。设置时 xRot 也必须提供。 */
  yRot?: string;
  /** 俯仰角 pitch。 */
  xRot?: string;
  /** 面朝某坐标点。三者需同时提供，优先于 yRot/xRot。 */
  facingX?: string;
  facingY?: string;
  facingZ?: string;
}

/** 传送到实体/玩家。 */
export interface TpEntityForm {
  withSlash?: boolean;
  useTeleportAlias?: boolean;
  targets: string;
  /** 目标实体选择器或玩家名。 */
  destination: string;
}

export function buildTpCommand(form: TpCoordsForm | TpEntityForm): string {
  const cmd = "destination" in form ? buildTpToEntity(form) : buildTpToCoords(form);
  return form.withSlash ? `/${cmd}` : cmd;
}

function buildTpToCoords(form: TpCoordsForm): string {
  const verb = form.useTeleportAlias ? "teleport" : "tp";
  const base = `${verb} ${form.targets} ${form.x} ${form.y} ${form.z}`;

  if (form.facingX !== undefined && form.facingY !== undefined && form.facingZ !== undefined) {
    return `${base} facing ${form.facingX} ${form.facingY} ${form.facingZ}`;
  }
  if (form.yRot !== undefined && form.xRot !== undefined) {
    return `${base} ${form.yRot} ${form.xRot}`;
  }
  return base;
}

function buildTpToEntity(form: TpEntityForm): string {
  const verb = form.useTeleportAlias ? "teleport" : "tp";
  return `${verb} ${form.targets} ${form.destination}`;
}
