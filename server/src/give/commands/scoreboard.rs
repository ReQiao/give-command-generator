//! `/scoreboard` 指令构建器。移植自客户端 `src/logic/commands/scoreboard.ts`。
//!
//! 语法（1.20.5+ 全版本一致）：
//!   scoreboard objectives add|remove|list|setdisplay|modify …
//!   scoreboard players set|add|remove|get|reset|enable|list|operation …
//!
//! displayName 自 1.20.3 起为文本组件——命令参数里直接写 JSON（如 {"text":"x"}），
//! 这一写法跨 1.20.5+ 所有版本一致，无需版本分支。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreboardOperation {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Gt,
    Swap,
}

impl ScoreboardOperation {
    fn as_str(self) -> &'static str {
        match self {
            ScoreboardOperation::Set => "=",
            ScoreboardOperation::Add => "+=",
            ScoreboardOperation::Sub => "-=",
            ScoreboardOperation::Mul => "*=",
            ScoreboardOperation::Div => "/=",
            ScoreboardOperation::Mod => "%=",
            ScoreboardOperation::Lt => "<",
            ScoreboardOperation::Gt => ">",
            ScoreboardOperation::Swap => "><",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderType {
    Hearts,
    Integer,
}

impl RenderType {
    fn as_str(self) -> &'static str {
        match self {
            RenderType::Hearts => "hearts",
            RenderType::Integer => "integer",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScoreboardAction {
    ObjectivesAdd { objective: String, criteria: String, display_name: Option<String> },
    ObjectivesRemove { objective: String },
    ObjectivesList,
    ObjectivesSetdisplay { slot: String, objective: Option<String> },
    ObjectivesModifyDisplayname { objective: String, display_name: String },
    ObjectivesModifyRendertype { objective: String, rendertype: RenderType },
    PlayersSet { targets: String, objective: String, score: i64 },
    PlayersAdd { targets: String, objective: String, score: i64 },
    PlayersRemove { targets: String, objective: String, score: i64 },
    PlayersGet { target: String, objective: String },
    PlayersReset { targets: String, objective: Option<String> },
    PlayersEnable { targets: String, objective: String },
    PlayersList { target: Option<String> },
    PlayersOperation {
        targets: String,
        objective: String,
        operation: ScoreboardOperation,
        source: String,
        source_objective: String,
    },
}

#[derive(Debug, Clone)]
pub struct ScoreboardForm {
    pub with_slash: bool,
    pub action: ScoreboardAction,
}

/// 把纯文本包成文本组件 JSON（命令参数里直接内联）。
fn text_component(text: &str) -> String {
    serde_json::json!({ "text": text }).to_string()
}

pub fn build_scoreboard_command(form: &ScoreboardForm) -> String {
    let cmd = match &form.action {
        ScoreboardAction::ObjectivesAdd { objective, criteria, display_name } => {
            let mut cmd = format!("scoreboard objectives add {objective} {criteria}");
            if let Some(name) = display_name {
                if !name.is_empty() {
                    cmd.push(' ');
                    cmd.push_str(&text_component(name));
                }
            }
            cmd
        }
        ScoreboardAction::ObjectivesRemove { objective } => format!("scoreboard objectives remove {objective}"),
        ScoreboardAction::ObjectivesList => "scoreboard objectives list".to_string(),
        ScoreboardAction::ObjectivesSetdisplay { slot, objective } => {
            let mut cmd = format!("scoreboard objectives setdisplay {slot}");
            if let Some(objective) = objective {
                if !objective.is_empty() {
                    cmd.push(' ');
                    cmd.push_str(objective);
                }
            }
            cmd
        }
        ScoreboardAction::ObjectivesModifyDisplayname { objective, display_name } => {
            format!("scoreboard objectives modify {objective} displayname {}", text_component(display_name))
        }
        ScoreboardAction::ObjectivesModifyRendertype { objective, rendertype } => {
            format!("scoreboard objectives modify {objective} rendertype {}", rendertype.as_str())
        }
        ScoreboardAction::PlayersSet { targets, objective, score } => {
            format!("scoreboard players set {targets} {objective} {score}")
        }
        ScoreboardAction::PlayersAdd { targets, objective, score } => {
            format!("scoreboard players add {targets} {objective} {score}")
        }
        ScoreboardAction::PlayersRemove { targets, objective, score } => {
            format!("scoreboard players remove {targets} {objective} {score}")
        }
        ScoreboardAction::PlayersGet { target, objective } => {
            format!("scoreboard players get {target} {objective}")
        }
        ScoreboardAction::PlayersReset { targets, objective } => {
            let mut cmd = format!("scoreboard players reset {targets}");
            if let Some(objective) = objective {
                if !objective.is_empty() {
                    cmd.push(' ');
                    cmd.push_str(objective);
                }
            }
            cmd
        }
        ScoreboardAction::PlayersEnable { targets, objective } => {
            format!("scoreboard players enable {targets} {objective}")
        }
        ScoreboardAction::PlayersList { target } => {
            let mut cmd = "scoreboard players list".to_string();
            if let Some(target) = target {
                if !target.is_empty() {
                    cmd.push(' ');
                    cmd.push_str(target);
                }
            }
            cmd
        }
        ScoreboardAction::PlayersOperation { targets, objective, operation, source, source_objective } => {
            format!(
                "scoreboard players operation {targets} {objective} {} {source} {source_objective}",
                operation.as_str()
            )
        }
    };

    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(action: ScoreboardAction) -> ScoreboardForm {
        ScoreboardForm { with_slash: false, action }
    }

    #[test]
    fn objectives_add() {
        let f = form(ScoreboardAction::ObjectivesAdd {
            objective: "kills".to_string(),
            criteria: "playerKillCount".to_string(),
            display_name: None,
        });
        assert_eq!(build_scoreboard_command(&f), "scoreboard objectives add kills playerKillCount");
    }

    #[test]
    fn objectives_add_with_display_name() {
        let f = form(ScoreboardAction::ObjectivesAdd {
            objective: "kills".to_string(),
            criteria: "dummy".to_string(),
            display_name: Some("击杀数".to_string()),
        });
        assert_eq!(
            build_scoreboard_command(&f),
            r#"scoreboard objectives add kills dummy {"text":"击杀数"}"#
        );
    }

    #[test]
    fn objectives_setdisplay() {
        let f = form(ScoreboardAction::ObjectivesSetdisplay {
            slot: "sidebar".to_string(),
            objective: Some("kills".to_string()),
        });
        assert_eq!(build_scoreboard_command(&f), "scoreboard objectives setdisplay sidebar kills");
    }

    #[test]
    fn players_set() {
        let f = form(ScoreboardAction::PlayersSet {
            targets: "@a".to_string(),
            objective: "kills".to_string(),
            score: 0,
        });
        assert_eq!(build_scoreboard_command(&f), "scoreboard players set @a kills 0");
    }

    #[test]
    fn players_operation() {
        let f = form(ScoreboardAction::PlayersOperation {
            targets: "@s".to_string(),
            objective: "a".to_string(),
            operation: ScoreboardOperation::Add,
            source: "@s".to_string(),
            source_objective: "b".to_string(),
        });
        assert_eq!(build_scoreboard_command(&f), "scoreboard players operation @s a += @s b");
    }

    #[test]
    fn players_reset_all() {
        let f = form(ScoreboardAction::PlayersReset { targets: "@a".to_string(), objective: None });
        assert_eq!(build_scoreboard_command(&f), "scoreboard players reset @a");
    }
}
