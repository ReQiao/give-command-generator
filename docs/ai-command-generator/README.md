# AI 指令生成器（新产品方向）

> 本分支（`move`）承载从「纯 /give 生成器」转向「AI 驱动的通用 MC 指令生成器」的全部改动。
> `main` 分支保留**组件时代全兼容的 /give 最终版**，不含本方向的代码。

## 这是什么

把工具从「只生成 /give」升级为**通用 MC 指令生成器**：用户用自然语言描述需求（例如「做一个能射 TNT 的弓」），
AI 理解后自动生成所需的全部 MC 命令，并通过桌面端 + 客户端 Mod 直接落地到游戏里。

详细产品规划、架构、变现与风险见 [PLAN.md](./PLAN.md)。

## 与 main 的关系

| 分支 | 内容 |
|------|------|
| `main` | 组件时代全兼容的 /give 最终版（单体 `builder.ts`，含 give-only 的 mc-verifier 工具） |
| `move`（本分支） | 在 main 基础上，转向多指令 / AI 方向的全部改动 |

## 本分支已完成的工作

### 1. Builder 架构重构（多指令基础）
单体 `builder.ts`（761 行）按职责拆分为模块，并保留 barrel 兼容旧导入：
- `types.ts` / `snbt.ts` / `version.ts` / `form.ts` / `catalog-util.ts` / `color.ts` / `util.ts`
- `commands/give.ts` — /give 构建器
- `commands/nbt.ts` — **跨指令共享**的 item-in-NBT / 属性 / 效果 / 装备序列化器

### 2. P1 指令 builder（1.20.6 ~ 1.21.5 全覆盖）
| 指令 | 文件 | 版本分派 |
|------|------|---------|
| `/give` | `commands/give.ts` | early / legacy / mid / modern |
| `/setblock` | `commands/setblock.ts` | 格式跨版本一致 |
| `/summon` | `commands/summon.ts` | 属性 + 装备两套（旧 vs 1.21.5+） |
| `/say` | `commands/say.ts` | 无差异 |
| `/effect` | `commands/effect.ts` | 无差异 |
| `/tp` | `commands/tp.ts` | 无差异 |

### 3. mc-verifier 扩展（语法 + 语义双层实证）
- `probes.mjs`：扩展到 P1 全部指令的语法探针，跨指令通用分类器（以 Brigadier `<--[HERE]` 为唯一非法信号）
- `semantic-probe.mjs`：summon/setblock → `data get` 读回真实存储的 NBT，裁决版本敏感的键名差异

### 4. 实测 NBT 真值表（1.20.6 vs 1.21.5）
| 特性 | 1.20.6（early/legacy/mid） | 1.21.5（modern） |
|------|--------------------------|-----------------|
| 属性键 | `Attributes[]/Name/Base` + `generic.` 前缀 | `attributes[]/id/base`，无前缀 |
| 装备槽 | `HandItems[]` / `ArmorItems[]` | `equipment{mainhand,...}` |
| CustomName | SNBT 字符串 | + 接受裸 JSON compound |
| 状态效果 | `active_effects[]` + 字符串 id（两版本一致） | 同左 |
| item-in-NBT | `{id,count,Slot,components}`（两版本一致） | 同左 |

测试：95 个快照测试全部通过（`npm test`）。

## 下一步

见 [PLAN.md](./PLAN.md) 的「下一步」：优先搭建 exe（Electron）骨架 + Mod（Fabric/Kotlin）HTTP 监听骨架，
打通 exe → Mod 通信链路。
