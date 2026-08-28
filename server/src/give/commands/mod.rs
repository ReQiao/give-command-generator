//! 多指令构建器集合。移植自客户端 `src/logic/commands/*.ts`。
//!
//! 每个子模块对应一个 TS 文件，构造出"意图数据结构 -> 指令字符串"的确定性拼接逻辑。
//! 目录存在性校验（拦截 AI 编造的 id）不属于这些模块的职责，那是 dispatch 层的事。

pub mod say;
pub mod enchant;
pub mod nbt;
pub mod effect;
pub mod tp;
pub mod fill;
pub mod clone;
pub mod execute;
pub mod particle;
pub mod setblock;
pub mod scoreboard;
pub mod attribute;
pub mod summon;
