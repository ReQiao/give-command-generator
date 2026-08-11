//! `/execute` 指令构建器。移植自客户端 `src/logic/commands/execute.ts`。
//!
//! 语法（1.20.5+ 全版本一致）：
//!   execute <subcommand>... run <command>
//!   execute <subcommand>...                  （以 if/unless 结尾，用于条件测试）
//!
//! 子命令自 1.13 重写后稳定，1.19.4 增补 on/summon/if biome/if dimension，
//! 1.20.5 增补 if items / store … items —— 1.20.5+ 全部可用，无版本分支。
//!
//! 设计：接收「已成形的子命令片段」数组（如 "as @a"、"at @s"），按序拼接并附加 run；
//! 同时提供常用子命令的构造器（`sub` 模块），降低 AI / 调用方拼错的概率。

#[derive(Debug, Clone, Default)]
pub struct ExecuteForm {
    pub with_slash: bool,
    /// 已成形的子命令片段，按顺序拼接。
    pub subcommands: Vec<String>,
    /// 最终执行的命令（前导斜杠会被去掉）。省略表示这是一条纯条件测试。
    pub run: Option<String>,
    /// 这条命令需要每 tick 持续侦测（例如箭矢/掉落物落地检测），不是执行一次就完事。
    /// 只是元数据，不影响生成的命令字符串本身——由 dispatch 往上传递给部署逻辑：
    /// 标记为 true 的命令会被自动写进 datapack 的 tick 循环（挂 tick.json），
    /// 而不是要求玩家自己找一个循环命令方块去放。
    pub r#loop: bool,
}

pub fn build_execute_command(form: &ExecuteForm) -> Result<String, String> {
    let subs: Vec<String> =
        form.subcommands.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if subs.is_empty() {
        return Err("execute 至少需要一个子命令。".to_string());
    }

    let mut parts: Vec<String> = vec!["execute".to_string()];
    parts.extend(subs.iter().cloned());

    match &form.run {
        Some(run) if !run.trim().is_empty() => {
            parts.push("run".to_string());
            let trimmed = run.trim();
            let stripped = trimmed.strip_prefix('/').unwrap_or(trimmed);
            parts.push(stripped.to_string());
        }
        _ => {
            let last = subs.last().unwrap();
            let starts_with_word = |s: &str, word: &str| {
                s.strip_prefix(word).is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
            };
            if !(starts_with_word(last, "if") || starts_with_word(last, "unless")) {
                // 无 run 时最后一个子命令必须是条件，否则服务器报错。
                return Err("execute 无 run 时必须以 if/unless 子命令结尾。".to_string());
            }
        }
    }

    let cmd = parts.join(" ");
    Ok(if form.with_slash { format!("/{cmd}") } else { cmd })
}

pub type Coords3 = [String; 3];

/// 常用子命令构造器（可选使用，返回子命令片段字符串）。
pub mod sub {
    use super::Coords3;

    pub fn r#as(selector: &str) -> String {
        format!("as {selector}")
    }
    pub fn at(selector: &str) -> String {
        format!("at {selector}")
    }
    pub fn positioned(pos: &Coords3) -> String {
        format!("positioned {}", pos.join(" "))
    }
    pub fn positioned_as(selector: &str) -> String {
        format!("positioned as {selector}")
    }
    pub fn rotated(yaw: &str, pitch: &str) -> String {
        format!("rotated {yaw} {pitch}")
    }
    pub fn rotated_as(selector: &str) -> String {
        format!("rotated as {selector}")
    }
    pub fn facing(pos: &Coords3) -> String {
        format!("facing {}", pos.join(" "))
    }
    pub fn facing_entity(selector: &str, anchor: Option<&str>) -> String {
        format!("facing entity {selector} {}", anchor.unwrap_or("eyes"))
    }
    pub fn anchored(anchor: &str) -> String {
        format!("anchored {anchor}")
    }
    pub fn r#in(dimension: &str) -> String {
        format!("in {dimension}")
    }
    pub fn on(relation: &str) -> String {
        format!("on {relation}")
    }
    pub fn align(axes: &str) -> String {
        format!("align {axes}")
    }

    pub fn if_block(pos: &Coords3, block: &str) -> String {
        format!("if block {} {block}", pos.join(" "))
    }
    pub fn unless_block(pos: &Coords3, block: &str) -> String {
        format!("unless block {} {block}", pos.join(" "))
    }
    pub fn if_entity(selector: &str) -> String {
        format!("if entity {selector}")
    }
    pub fn unless_entity(selector: &str) -> String {
        format!("unless entity {selector}")
    }
    pub fn if_score_matches(target: &str, objective: &str, range: &str) -> String {
        format!("if score {target} {objective} matches {range}")
    }
    pub fn if_predicate(predicate: &str) -> String {
        format!("if predicate {predicate}")
    }

    pub fn store_result_score(target: &str, objective: &str) -> String {
        format!("store result score {target} {objective}")
    }
    pub fn store_success_score(target: &str, objective: &str) -> String {
        format!("store success score {target} {objective}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_at_run() {
        let form = ExecuteForm {
            subcommands: vec![sub::r#as("@a"), sub::at("@s")],
            run: Some("say hi".to_string()),
            ..Default::default()
        };
        assert_eq!(build_execute_command(&form).unwrap(), "execute as @a at @s run say hi");
    }

    #[test]
    fn run_leading_slash_stripped() {
        let form = ExecuteForm {
            subcommands: vec![sub::at("@s")],
            run: Some("/summon minecraft:tnt ~ ~ ~".to_string()),
            ..Default::default()
        };
        assert_eq!(build_execute_command(&form).unwrap(), "execute at @s run summon minecraft:tnt ~ ~ ~");
    }

    #[test]
    fn pure_condition_no_run_ends_with_if() {
        let form =
            ExecuteForm { subcommands: vec![sub::if_entity("@e[type=pig]")], ..Default::default() };
        assert_eq!(build_execute_command(&form).unwrap(), "execute if entity @e[type=pig]");
    }

    #[test]
    fn arrow_ground_detection() {
        let form = ExecuteForm {
            subcommands: vec![sub::at("@e[type=arrow,nbt={inGround:1b}]")],
            run: Some("summon tnt ~ ~ ~".to_string()),
            ..Default::default()
        };
        assert_eq!(
            build_execute_command(&form).unwrap(),
            "execute at @e[type=arrow,nbt={inGround:1b}] run summon tnt ~ ~ ~"
        );
    }

    #[test]
    fn no_subcommands_errors() {
        let form = ExecuteForm { subcommands: vec![], ..Default::default() };
        assert!(build_execute_command(&form).is_err());
    }

    #[test]
    fn no_run_not_ending_if_unless_errors() {
        let form = ExecuteForm { subcommands: vec![sub::r#as("@a")], ..Default::default() };
        assert!(build_execute_command(&form).is_err());
    }
}
