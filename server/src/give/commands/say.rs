//! `/say` 指令构建器。移植自客户端 `src/logic/commands/say.ts`。
//!
//! 语法（全版本一致）：say <message>
//! message 是纯文本，选择器（@a 等）会被展开为玩家名列表。

#[derive(Debug, Clone, Default)]
pub struct SayForm {
    pub with_slash: bool,
    pub message: String,
}

pub fn build_say_command(form: &SayForm) -> String {
    let cmd = format!("say {}", form.message);
    if form.with_slash { format!("/{cmd}") } else { cmd }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let form = SayForm { with_slash: false, message: "hello world".to_string() };
        assert_eq!(build_say_command(&form), "say hello world");
    }

    #[test]
    fn with_slash() {
        let form = SayForm { with_slash: true, message: "hi".to_string() };
        assert_eq!(build_say_command(&form), "/say hi");
    }
}
