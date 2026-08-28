//! `/effect` 指令构建器。移植自客户端 `src/logic/commands/effect.ts`。
//!
//! 语法（1.19.4+ 全版本一致，覆盖 1.20.5 ~ 26.x）：
//!   effect give <target> <effect> [<seconds>|infinite] [<amplifier>] [<hideParticles>]
//!   effect clear <target> [<effect>]
//!
//! mc-verifier 探针实测（1.20.6 / 1.21.5 均有效）：
//!   effect give @a minecraft:speed / …30 / …30 2 / …30 2 true / …infinite 1
//!   effect clear @a / effect clear @a minecraft:speed

use crate::give::catalog::namespaced;

/// 时长：具体秒数或 "infinite"。
#[derive(Debug, Clone)]
pub enum EffectDuration {
    Seconds(i64),
    Infinite,
}

#[derive(Debug, Clone, Default)]
pub struct EffectGiveForm {
    pub with_slash: bool,
    pub target: String,
    pub effect: String,
    /// 时长（秒）。传 Infinite 表示无限。省略时服务器默认 30 秒。
    pub duration: Option<EffectDuration>,
    /// 效果等级，0 = I，1 = II，依此类推。省略时服务器默认 0（I 级）。
    pub amplifier: Option<i64>,
    /// 隐藏粒子效果。省略时服务器默认 false（显示粒子）。
    pub hide_particles: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct EffectClearForm {
    pub with_slash: bool,
    pub target: String,
    /// 省略时清除该目标所有效果。
    pub effect: Option<String>,
}

pub fn build_effect_give_command(form: &EffectGiveForm) -> String {
    let mut parts: Vec<String> = vec![format!("effect give {} {}", form.target, namespaced(&form.effect))];

    // duration / amplifier / hideParticles 是位置参数：后面的要写，前面的就必须显式补出。
    let has_duration = form.duration.is_some();
    let has_amplifier = form.amplifier.is_some();
    let has_hide = form.hide_particles.is_some();

    if has_duration || has_amplifier || has_hide {
        let duration_str = match &form.duration {
            Some(EffectDuration::Seconds(s)) => s.to_string(),
            Some(EffectDuration::Infinite) => "infinite".to_string(),
            None => "30".to_string(),
        };
        parts.push(duration_str);
    }
    if has_amplifier || has_hide {
        parts.push(form.amplifier.unwrap_or(0).to_string());
    }
    if has_hide {
        parts.push(if form.hide_particles.unwrap_or(false) { "true".to_string() } else { "false".to_string() });
    }

    let cmd = parts.join(" ");
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

pub fn build_effect_clear_command(form: &EffectClearForm) -> String {
    let mut cmd = format!("effect clear {}", form.target);
    if let Some(effect) = &form.effect {
        if !effect.is_empty() {
            cmd.push(' ');
            cmd.push_str(&namespaced(effect));
        }
    }
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn give_effect_only() {
        let form = EffectGiveForm { target: "@a".to_string(), effect: "speed".to_string(), ..Default::default() };
        assert_eq!(build_effect_give_command(&form), "effect give @a minecraft:speed");
    }

    #[test]
    fn give_with_duration() {
        let form = EffectGiveForm {
            target: "@a".to_string(),
            effect: "speed".to_string(),
            duration: Some(EffectDuration::Seconds(30)),
            ..Default::default()
        };
        assert_eq!(build_effect_give_command(&form), "effect give @a minecraft:speed 30");
    }

    #[test]
    fn give_with_amplifier_duration_defaulted() {
        let form = EffectGiveForm {
            target: "@a".to_string(),
            effect: "speed".to_string(),
            amplifier: Some(2),
            ..Default::default()
        };
        assert_eq!(build_effect_give_command(&form), "effect give @a minecraft:speed 30 2");
    }

    #[test]
    fn give_with_hide_particles_all_leading_defaulted() {
        let form = EffectGiveForm {
            target: "@a".to_string(),
            effect: "speed".to_string(),
            hide_particles: Some(true),
            ..Default::default()
        };
        assert_eq!(build_effect_give_command(&form), "effect give @a minecraft:speed 30 0 true");
    }

    #[test]
    fn give_infinite() {
        let form = EffectGiveForm {
            target: "@a".to_string(),
            effect: "speed".to_string(),
            duration: Some(EffectDuration::Infinite),
            amplifier: Some(1),
            ..Default::default()
        };
        assert_eq!(build_effect_give_command(&form), "effect give @a minecraft:speed infinite 1");
    }

    #[test]
    fn clear_all() {
        let form = EffectClearForm { target: "@a".to_string(), ..Default::default() };
        assert_eq!(build_effect_clear_command(&form), "effect clear @a");
    }

    #[test]
    fn clear_specific() {
        let form = EffectClearForm {
            target: "@a".to_string(),
            effect: Some("minecraft:speed".to_string()),
            ..Default::default()
        };
        assert_eq!(build_effect_clear_command(&form), "effect clear @a minecraft:speed");
    }
}
