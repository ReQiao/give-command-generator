# Java 1.21.5 / 1.21.6~1.21.8 / 1.21.9~1.21.10 / 26.1 / 26.2+ give 语法

## 来源与验证

- 验证方式：本仓库 `scripts/mc-verifier`，用 Mojang 官方 server.jar + RCON 实测。
- 验证版本：1.21.5、1.21.6、1.21.9（各代表其所在族）。
- 26.1 / 26.2+ 需要 Java 25+，当前环境无法验证，暂按 modern 处理。
- 验证日期：2026-06。
- 原始结果：`scripts/mc-verifier/results/1.21.5/`、`results/1.21.6/`、`results/1.21.9/`。

## 版本族归属

1.21.5 / 1.21.6~1.21.8 / 1.21.9~1.21.10 / 26.1 / 26.2+ 均属于 **modern 族**，
与 1.21.11 完全一致（PASS=21 FAIL=0）：

builder 路由：默认 `buildModernFamily(form, MODERN_PROFILE)`。

## 与 1.21.11 对比

服务器实证结果：三个代表版本（1.21.5、1.21.6、1.21.9）的每条探针结果与 1.21.11 完全相同。
包括：
- 文本：直接 JSON 数组
- enchantments：扁平形式
- attribute_modifiers：数组形式，type 不带引号
- can_place_on / can_break：直接引号列表 `[{blocks:"..."}]`
- 支持 consumable / glider / death_protection
- 支持 tooltip_display

## 版本边界

经验证，mid→modern 的语法切换发生在 **1.21.4 → 1.21.5** 之间：
- 1.21.4：mid 族（SNBT 文本、predicates 包装、无 tooltip）
- 1.21.5：modern 族（直接 JSON、直接列表、支持 tooltip）

## 对应测试

- 单元/快照：`src/logic/builder.test.mjs`（用例 26–29）。
- 服务器回归：`npm run verify-syntax -- 1.21.5 1.21.6 1.21.9`。
