//! `/particle` 指令构建器。移植自客户端 `src/logic/commands/particle.ts`。
//!
//! 语法（全版本一致）：
//!   particle <name> [<pos>] [<delta> <speed> <count> [force|normal] [<viewers>]]
//!
//! 参数化粒子的附加数据写在粒子 id 后面、紧跟无空格，例如：
//!   minecraft:dust{color:[1.0,0.0,0.0],scale:1.0}                  — 红色尘埃，1.20.5+
//!   minecraft:dust_color_transition{from_color:[...],to_color:[...],scale:1.0}
//!   minecraft:block{block_state:{Name:"minecraft:stone"}}          — 方块碎裂粒子
//!   minecraft:item{item:{id:"minecraft:diamond",count:1}}          — 物品图标粒子
//! 这类粒子把完整 id（含附加数据）传进 name 字段即可，构建器只在花括号前的部分补
//! minecraft: 前缀，不会破坏花括号内的内容。

use crate::give::builder::fmt_number_f64;
use crate::give::catalog::namespaced;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleMode {
    Force,
    Normal,
}

impl ParticleMode {
    fn as_str(self) -> &'static str {
        match self {
            ParticleMode::Force => "force",
            ParticleMode::Normal => "normal",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParticleForm {
    pub with_slash: bool,
    /// 粒子 id，可携带参数化附加数据（见文件头示例）。
    pub name: String,
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
    /// 生成范围（每个方向的随机偏移半径）。
    pub dx: Option<f64>,
    pub dy: Option<f64>,
    pub dz: Option<f64>,
    /// 粒子速度/运动参数，含义因粒子类型而异。
    pub speed: Option<f64>,
    pub count: Option<i64>,
    /// force：无视客户端粒子设置和距离限制都显示；normal：按客户端设置和默认距离。
    pub mode: Option<ParticleMode>,
    /// 只让指定玩家看到，省略则视距内所有玩家可见。
    pub viewers: Option<String>,
}

/// 只给花括号前的粒子 id 部分补 minecraft: 前缀，避免误伤附加数据里的冒号。
fn namespaced_particle_id(raw: &str) -> String {
    let text = raw.trim();
    match text.find('{') {
        None => namespaced(text),
        Some(brace_index) => format!("{}{}", namespaced(&text[..brace_index]), &text[brace_index..]),
    }
}

pub fn build_particle_command(form: &ParticleForm) -> String {
    let mut parts: Vec<String> = vec![format!("particle {}", namespaced_particle_id(&form.name))];

    let has_pos = form.x.is_some() && form.y.is_some() && form.z.is_some();
    let has_extra = form.dx.is_some()
        || form.dy.is_some()
        || form.dz.is_some()
        || form.speed.is_some()
        || form.count.is_some()
        || form.mode.is_some()
        || form.viewers.is_some();

    if has_pos || has_extra {
        parts.push(form.x.clone().unwrap_or_else(|| "~".to_string()));
        parts.push(form.y.clone().unwrap_or_else(|| "~".to_string()));
        parts.push(form.z.clone().unwrap_or_else(|| "~".to_string()));
    }
    if has_extra {
        parts.push(fmt_number_f64(form.dx.unwrap_or(0.0)));
        parts.push(fmt_number_f64(form.dy.unwrap_or(0.0)));
        parts.push(fmt_number_f64(form.dz.unwrap_or(0.0)));
        parts.push(fmt_number_f64(form.speed.unwrap_or(1.0)));
        parts.push(form.count.unwrap_or(1).to_string());
        if form.mode.is_some() || form.viewers.is_some() {
            parts.push(form.mode.unwrap_or(ParticleMode::Normal).as_str().to_string());
        }
        if let Some(viewers) = &form.viewers {
            parts.push(viewers.clone());
        }
    }

    let cmd = parts.join(" ");
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplest_form() {
        let form = ParticleForm { name: "flame".to_string(), ..Default::default() };
        assert_eq!(build_particle_command(&form), "particle minecraft:flame");
    }

    #[test]
    fn with_pos_and_extra() {
        let form = ParticleForm {
            name: "flame".to_string(),
            x: Some("~".to_string()),
            y: Some("~1".to_string()),
            z: Some("~".to_string()),
            dx: Some(0.3),
            dy: Some(0.3),
            dz: Some(0.3),
            speed: Some(0.02),
            count: Some(20),
            ..Default::default()
        };
        assert_eq!(build_particle_command(&form), "particle minecraft:flame ~ ~1 ~ 0.3 0.3 0.3 0.02 20");
    }

    #[test]
    fn force_mode() {
        let form = ParticleForm {
            name: "totem_of_undying".to_string(),
            x: Some("~".to_string()),
            y: Some("~1".to_string()),
            z: Some("~".to_string()),
            dx: Some(0.5),
            dy: Some(0.5),
            dz: Some(0.5),
            speed: Some(0.0),
            count: Some(100),
            mode: Some(ParticleMode::Force),
            ..Default::default()
        };
        assert_eq!(
            build_particle_command(&form),
            "particle minecraft:totem_of_undying ~ ~1 ~ 0.5 0.5 0.5 0 100 force"
        );
    }

    #[test]
    fn viewers_auto_normal() {
        let form = ParticleForm {
            name: "flame".to_string(),
            x: Some("~".to_string()),
            y: Some("~".to_string()),
            z: Some("~".to_string()),
            count: Some(5),
            viewers: Some("@a".to_string()),
            ..Default::default()
        };
        assert_eq!(build_particle_command(&form), "particle minecraft:flame ~ ~ ~ 0 0 0 1 5 normal @a");
    }

    #[test]
    fn parametric_dust_particle() {
        let form = ParticleForm {
            name: "dust{color:[1.0,0.2,0.2],scale:1.5}".to_string(),
            x: Some("~".to_string()),
            y: Some("~".to_string()),
            z: Some("~".to_string()),
            count: Some(10),
            ..Default::default()
        };
        assert_eq!(
            build_particle_command(&form),
            "particle minecraft:dust{color:[1.0,0.2,0.2],scale:1.5} ~ ~ ~ 0 0 0 1 10"
        );
    }

    #[test]
    fn with_slash() {
        let form = ParticleForm { name: "flame".to_string(), with_slash: true, ..Default::default() };
        assert_eq!(build_particle_command(&form), "/particle minecraft:flame");
    }
}
