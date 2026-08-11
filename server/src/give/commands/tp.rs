//! `/tp`（`/teleport`）指令构建器。移植自客户端 `src/logic/commands/tp.ts`。
//!
//! 语法（全版本一致，覆盖 1.20.5 ~ 26.x）：
//!   tp <targets> <x> <y> <z>                          — 传送到坐标
//!   tp <targets> <x> <y> <z> <yRot> <xRot>            — 传送到坐标并设定朝向
//!   tp <targets> <x> <y> <z> facing <fx> <fy> <fz>    — 传送到坐标并面朝某点
//!   tp <targets> <destination>                        — 传送到实体/玩家
//!
//! 坐标支持绝对（0）、相对（~、~1）、本地（^、^1）写法。

/// 传送到坐标（可选朝向）。
#[derive(Debug, Clone, Default)]
pub struct TpCoordsForm {
    pub with_slash: bool,
    /// 是否用 teleport 别名（默认 tp）。
    pub use_teleport_alias: bool,
    pub targets: String,
    pub x: String,
    pub y: String,
    pub z: String,
    /// 偏航角 yaw。设置时 xRot 也必须提供。
    pub y_rot: Option<String>,
    /// 俯仰角 pitch。
    pub x_rot: Option<String>,
    /// 面朝某坐标点。三者需同时提供，优先于 yRot/xRot。
    pub facing_x: Option<String>,
    pub facing_y: Option<String>,
    pub facing_z: Option<String>,
}

/// 传送到实体/玩家。
#[derive(Debug, Clone, Default)]
pub struct TpEntityForm {
    pub with_slash: bool,
    pub use_teleport_alias: bool,
    pub targets: String,
    /// 目标实体选择器或玩家名。
    pub destination: String,
}

#[derive(Debug, Clone)]
pub enum TpForm {
    Coords(TpCoordsForm),
    Entity(TpEntityForm),
}

pub fn build_tp_command(form: &TpForm) -> String {
    let (cmd, with_slash) = match form {
        TpForm::Entity(f) => (build_tp_to_entity(f), f.with_slash),
        TpForm::Coords(f) => (build_tp_to_coords(f), f.with_slash),
    };
    if with_slash { format!("/{cmd}") } else { cmd }
}

fn build_tp_to_coords(form: &TpCoordsForm) -> String {
    let verb = if form.use_teleport_alias { "teleport" } else { "tp" };
    let base = format!("{verb} {} {} {} {}", form.targets, form.x, form.y, form.z);

    if let (Some(fx), Some(fy), Some(fz)) = (&form.facing_x, &form.facing_y, &form.facing_z) {
        return format!("{base} facing {fx} {fy} {fz}");
    }
    if let (Some(y_rot), Some(x_rot)) = (&form.y_rot, &form.x_rot) {
        return format!("{base} {y_rot} {x_rot}");
    }
    base
}

fn build_tp_to_entity(form: &TpEntityForm) -> String {
    let verb = if form.use_teleport_alias { "teleport" } else { "tp" };
    format!("{verb} {} {}", form.targets, form.destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coords(targets: &str, x: &str, y: &str, z: &str) -> TpCoordsForm {
        TpCoordsForm {
            targets: targets.to_string(),
            x: x.to_string(),
            y: y.to_string(),
            z: z.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn absolute_coords() {
        let form = TpForm::Coords(coords("@s", "0", "64", "0"));
        assert_eq!(build_tp_command(&form), "tp @s 0 64 0");
    }

    #[test]
    fn relative_coords() {
        let form = TpForm::Coords(coords("@s", "~", "~", "~"));
        assert_eq!(build_tp_command(&form), "tp @s ~ ~ ~");
    }

    #[test]
    fn local_coords() {
        let form = TpForm::Coords(coords("@s", "^", "^", "^1"));
        assert_eq!(build_tp_command(&form), "tp @s ^ ^ ^1");
    }

    #[test]
    fn rotation() {
        let mut c = coords("@s", "0", "64", "0");
        c.y_rot = Some("90".to_string());
        c.x_rot = Some("45".to_string());
        assert_eq!(build_tp_command(&TpForm::Coords(c)), "tp @s 0 64 0 90 45");
    }

    #[test]
    fn facing_overrides_rotation() {
        let mut c = coords("@s", "0", "64", "0");
        c.y_rot = Some("90".to_string());
        c.x_rot = Some("45".to_string());
        c.facing_x = Some("10".to_string());
        c.facing_y = Some("70".to_string());
        c.facing_z = Some("10".to_string());
        assert_eq!(build_tp_command(&TpForm::Coords(c)), "tp @s 0 64 0 facing 10 70 10");
    }

    #[test]
    fn teleport_to_entity() {
        let form = TpForm::Entity(TpEntityForm {
            targets: "@s".to_string(),
            destination: "@e[type=pig,limit=1]".to_string(),
            ..Default::default()
        });
        assert_eq!(build_tp_command(&form), "tp @s @e[type=pig,limit=1]");
    }

    #[test]
    fn teleport_alias() {
        let mut c = coords("@s", "0", "64", "0");
        c.use_teleport_alias = true;
        assert_eq!(build_tp_command(&TpForm::Coords(c)), "teleport @s 0 64 0");
    }
}
