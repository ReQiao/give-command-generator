# Java 1.21.2 / 1.21.3 give 语法

## 来源与验证

- 验证方式：本仓库 `scripts/mc-verifier`，用 Mojang 官方 server.jar + RCON 实测。
- 验证版本：1.21.2、1.21.3（探针结果逐项完全一致，确认两者语法相同）。
- 验证日期：2026-06。
- 原始结果：`scripts/mc-verifier/results/1.21.2/`、`scripts/mc-verifier/results/1.21.3/`。
- 外部生成器对照：GamerGeeks / MCStacker 为客户端渲染 SPA，当前环境无法抓取，故以服务器实证为准。

## 版本族归属

1.21.2 / 1.21.3 属于 **mid 族**，是 legacy(1.21/1.21.1) 与 modern(1.21.11+) 之间的过渡形态：
以 modern 为主，但文本与方块限制回退到 legacy 写法，且不支持 tooltip_display。

builder 路由：`isJava1212Family(version)` -> `buildModernFamily(form, JAVA_1_21_2_PROFILE)`。

## 组件差异表（server 实证）

| 组件 | 1.21.2/1.21.3 输出 | 与 legacy 比 | 与 modern 比 |
|------|--------------------|--------------|--------------|
| custom_name / item_name / lore | SNBT 单引号字符串 | 相同 | 不同（modern 用直接 JSON） |
| rarity / enchantment_glint_override | 同各版本 | 相同 | 相同 |
| enchantments | 扁平 `{unbreaking:3}` | 不同（legacy 用 `{levels:{...}}`） | 相同 |
| attribute_modifiers | `[{type:armor,amount,slot?,id:"...",operation}]` | 不同（legacy 用 `{modifiers:[...]}` + `"generic.armor"`） | 相同 |
| can_place_on / can_break | `{predicates:[{blocks:"minecraft:stone"}]}` | 相同 | 不同（modern 用 `[{blocks:"..."}]`） |
| food | `{nutrition,saturation,can_always_eat?}` | 不同（legacy 并入 eat_seconds/effects） | 相同 |
| consumable | 独立组件，含 consume_seconds/sound/has_consume_particles/on_consume_effects | 不同（legacy 无独立 consumable） | 相同 |
| tool | `{...,rules:[{blocks:[stone],speed:Xf,correct_for_drops:1b}]}` | 相同 | 相同 |
| unbreakable / damage / max_damage / max_stack_size / repair_cost | 同各版本 | 相同 | 相同 |

## 支持组件

```
custom_name  item_name  lore  rarity  enchantment_glint_override
enchantments  attribute_modifiers  can_place_on  can_break
unbreakable  glider  death_protection
damage  max_damage  max_stack_size  repair_cost
food  consumable  tool
```

## 不支持 / 已省略组件

```
tooltip_display / hidden_components   （服务器返回 Unknown item component）
```

注意：enchantments 在 1.21.2/1.21.3 同时接受 `{levels:{...}}` 与扁平 `{...}`，
builder 统一输出扁平形式以与 modern 对齐。

## 对应测试

- 单元/快照：`src/logic/builder.test.mjs`（用例 18–24，覆盖文本、enchantments、
  attribute、can_place_on/break、glider/death_protection/consumable、tooltip 省略、
  以及 1.21.3 与 1.21.2 输出一致）。
- 服务器回归：`npm run verify-syntax -- 1.21.2 1.21.3`。
