//! AI 模式的 Builder 逻辑（意图校验 + 确定性指令构建），从客户端
//! `src/logic/{dispatch.ts,builder.ts,commands/*,ai/prompt.ts}` 移植过来。
//!
//! 迁移进行中——见仓库里的迁移计划。当前已完成：catalog 数据 + 存在性校验
//! （`catalog.rs`）、`GiveVersion` + `buildGiveCommand` 本体（`builder.rs`）、
//! 13 个 commands/* 构建器（`commands/`）、dispatch 分派与目录校验
//! （`dispatch.rs`）、`parseAiContent`（`parse.rs`）。尚未完成：
//! `server/src/main.rs` 的 `/v1/ai/generate` 接入这套逻辑（目前仍维持
//! "透传原始 AI 内容给客户端"的现状）。

pub mod builder;
pub mod catalog;
mod catalog_data;
pub mod commands;
pub mod dispatch;
pub mod parse;
