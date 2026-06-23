/**
 * /tp（/teleport）指令构建器。
 *
 * 语法（全版本一致，覆盖 1.20.6 ~ 1.21.5）：
 *   tp <targets> <x> <y> <z>                          — 传送到坐标
 *   tp <targets> <x> <y> <z> <yRot> <xRot>            — 传送到坐标并设定朝向
 *   tp <targets> <x> <y> <z> facing <fx> <fy> <fz>    — 传送到坐标并面朝某点
 *   tp <targets> <destination>                         — 传送到实体/玩家
 *
 * 坐标支持绝对（0）、相对（~、~1）、本地（^、^1）写法。
 * 1.20.6 ~ 1.21.5 语法完全一致，无版本差异。
 *
 * 实测探针验证（1.20.6 / 1.21.5 均有效）：
 *   tp @s 0 64 0
 *   tp @s ~ ~ ~
 *   tp @s ^ ^ ^1
 *   tp @s 0 64 0 90 45
 *   tp @s 0 64 0 facing 10 70 10
 *   tp @s @e[type=pig,limit=1]
 *   teleport @s 0 64 0
 */

// -----------------------------------------------------------------------
// 表单类型
// -----------------------------------------------------------------------

/** 传送到坐标（可选朝向）。 */
export interface TpCoordsForm {
  withSlash?: boolean;
  /** 是否用 teleport 别名（默认 tp）。 */
  useTeleportAlias?: boolean;
  targets: string;
  x: string;
  y: string;
  z: string;
  /** 朝向（偏航角 yaw）。设置时 xRot 也必须提供。 */
  yRot?: string;
  /** 朝向（俯仰角 pitch）。 */
  xRot?: string;
  /** 面朝某坐标点（x）。设置时 facingY / facingZ 也必须提供。 */
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

// -----------------------------------------------------------------------
// 主入口
// -----------------------------------------------------------------------

export function buildTpCommand(form: TpCoordsForm | TpEntityForm): string {
  const cmd = "destination" in form
    ? buildTpToEntity(form as TpEntityForm)
    : buildTpToCoords(form as TpCoordsForm);
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
