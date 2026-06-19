# MC give 语法自动验证器

用真实 Minecraft 官方服务器自动验证 `/give` 命令组件语法，为 `src/logic/builder.ts`
提供**服务器实证**的语法真相。可无人值守批量跑多个版本（适合过夜运行）。

## 原理

Minecraft 服务器执行 `give @a ...` 时**先解析语法，再查找目标玩家**。
因此无需任何玩家加入：

- 语法合法但无玩家 → `No player was found`
- 语法非法 → `Unknown argument` / `Expected ']'` / `Malformed ... component` / `<--[HERE]`

验证器通过 RCON 发送探针命令，根据响应判定每种写法是否被服务器接受。

## 用法

```bash
# 列出可用 release 版本
node scripts/mc-verifier/index.mjs --list

# 验证单个或多个版本
node scripts/mc-verifier/index.mjs 1.21.1
node scripts/mc-verifier/index.mjs 1.21 1.21.1 1.21.2 1.21.4 1.21.11

# 也可通过 npm
npm run verify-syntax -- 1.21.1
```

前提：

- 本机可用 `java`（1.21.x 需要 Java 21+）
- 可访问 `piston-meta.mojang.com`（下载 server.jar）
- 每个版本 server.jar 约 50MB，缓存在 `cache/`（已被 .gitignore 忽略）

## 输出

每个版本写入 `results/<version>/`：

- `raw.json`：每条探针的命令、原始服务器响应、分类结果
- `report.json`：按特性聚合，含 builder 当前输出是否被接受（PASS/FAIL/N/A）
- `report.txt`：人类可读摘要

### 报告判定

- `PASS`：builder.ts 对该版本族实际输出的格式被服务器接受
- `FAIL`：builder.ts 的某个实际输出格式被服务器拒绝（需修正 builder）
- `N/A`：builder.ts 对该版本族不输出此特性（仅记录服务器是否支持）
- 候选行内 `*` 标记表示该版本族 builder **实际会输出**这种格式

版本 → builder 族映射见 `probes.mjs` 的 `familyOf()`：
`1.21` / `1.21.1` 为 `legacy`，其余 Java 版默认 `modern`。

## 文件结构

| 文件 | 职责 |
|------|------|
| `index.mjs` | 主入口：编排下载 → 启动 → 探针 → 报告 |
| `mojang.mjs` | 从 Mojang 清单 API 下载并校验 server.jar |
| `server.mjs` | 服务器进程生命周期（临时工作目录、超平坦世界、RCON 配置） |
| `rcon.mjs` | 纯 Node.js Source RCON 客户端，无外部依赖 |
| `probes.mjs` | 探针集（特性 → 多候选格式）与响应分类器 |
| `report.mjs` | 结果聚合与文本摘要 |

## 扩展探针

在 `probes.mjs` 的 `PROBES` 数组添加条目：

```js
{
  feature: "某特性",          // 同特性的多个候选会归为一组
  id: "唯一标识",
  command: g('组件=值'),      // g() 自动包成 give @a minecraft:stone[...] 1
  builderFamilies: ["legacy"],// builder.ts 当前对哪些族输出该格式（驱动 PASS/FAIL）
  note: "说明",
}
```

耐久相关组件（max_damage 等）用 `gd()`（不可堆叠物品），避免触发
"Item cannot be both damageable and stackable" 这类物品约束错误，
保证失败只来自语法本身。

## 已知验证结论（1.21.1，server 实证）

- ✅ 确认正确：custom_name/item_name/lore 的 SNBT 单引号字符串、enchantments
  `{levels:{...}}`、attribute_modifiers `{modifiers:[{type:"generic.armor",...}]}`、
  food 含 `eat_seconds`/`effects`、tool `{rules:[{blocks:[stone],speed:Xf,correct_for_drops:1b}]}`
- ❌ builder 待修正：
  - `can_place_on` / `can_break`：当前输出 `[{blocks:...}]` 被拒，正确为
    `{predicates:[{blocks:"minecraft:stone"}]}`（命名空间 id 需引号）
  - 属性修饰符 `id` 为纯数字（如 `id:123`）被拒，必须是带引号的资源路径字符串
- ⛔ 1.21.1 不支持（builder legacy 已正确省略）：consumable、glider、
  death_protection、tooltip_display
