# Java 1.20.5 / 1.20.6 give 语法

## 来源与验证

- 验证方式：本仓库 `scripts/mc-verifier`，用 Mojang 官方 server.jar + RCON 实测。
- 验证版本：1.20.5（1.20.6 由用户确认语法相同）。
- 验证日期：2026-06。
- 原始结果：`scripts/mc-verifier/results/1.20.5/`。

## 版本族归属

1.20.5 / 1.20.6 属于 **early 族**，是组件时代（1.20.5+）的最初形态：
- 文本格式与 legacy/mid 相同（SNBT 单引号字符串）
- can_place_on/can_break 与 legacy/mid 相同（predicates 包装）
- 不支持 consumable、glider、death_protection、tooltip_display
- attribute_modifiers：所有已知格式均被服务器拒绝，暂不输出

builder 路由：`isJava1205Family(version)` -> `buildModernFamily(form, JAVA_1_20_5_PROFILE)`。

## 组件差异表（server 实证）

| 组件 | 1.20.5/1.20.6 | 与 mid 比 | 与 modern 比 |
|------|--------------|-----------|--------------|
| custom_name / item_name / lore | SNBT 单引号字符串 | 相同 | 不同 |
| rarity / enchantment_glint_override | 同各版本 | 相同 | 相同 |
| enchantments | 扁平 `{unbreaking:3}` | 相同 | 相同 |
| attribute_modifiers | **不输出**（所有格式均被拒绝） | 不同 | 不同 |
| can_place_on / can_break | `{predicates:[{blocks:"minecraft:stone"}]}` | 相同 | 不同 |
| food | `{nutrition,saturation,can_always_eat?}` | 相同 | 相同 |
| consumable | **不支持**（服务器返回错误） | 不同 | 不同 |
| glider | **不支持** | 不同 | 不同 |
| death_protection | **不支持** | 不同 | 不同 |
| tooltip_display | **不支持** | 相同 | 不同 |
| unbreakable / damage / max_damage / max_stack_size / repair_cost | 同各版本 | 相同 | 相同 |

## 支持组件

```
custom_name  item_name  lore  rarity  enchantment_glint_override
enchantments  can_place_on  can_break
unbreakable  damage  max_damage  max_stack_size  repair_cost
food  tool
```

## 不支持 / 已省略组件

```
attribute_modifiers   （所有已知格式均被服务器拒绝，格式待考证）
consumable            （服务器返回 Unknown item component）
glider                （服务器返回 Unknown item component）
death_protection      （服务器返回 Unknown item component）
tooltip_display       （服务器返回 Unknown item component）
```

## 对应测试

- 单元/快照：`src/logic/builder.test.mjs`（用例 30–35，覆盖文本、enchantments、
  can_place_on/break predicates、不输出 consumable/glider/death_protection/attribute/tooltip）。
- 服务器回归：`npm run verify-syntax -- 1.20.5`。
