# 方案：AI 驱动 MC 指令生成器（新产品方向）

> 注：原 MC verifier 工具已完成并合入 `claude/funny-cori-toj1l0`。此方案是下一阶段新产品规划。

## 用户愿景

不只是 /give——做成通用 MC 指令生成器：用户用自然语言描述（"做一个能射 TNT 的弓"），AI 理解并自动：
1. 生成所有需要的 MC 命令
2. 在游戏内放下命令方块（含指令）
3. 给玩家 give 对应物品

按调用量收费，定价高于 Qwen API 成本，收差价。

## 架构

```
用户自然语言输入
       ↓
[桌面 exe（Electron / Node.js）]
  - UI 对话框
  - 调用 Qwen API（~0.01元/次）
  - 用 builder.ts 处理结果，生成命令字符串
  - POST 到 localhost:25580
       ↓
[Fabric Mod（客户端，Kotlin，~200行）]
  - 嵌入 tiny HTTP 服务器，监听 localhost:25580
  - 收到命令字符串后，用 ClientCommandManager 执行
  - 检测玩家是否 OP，不是则提示
```

**关键设计原则：**
- Mod 是纯接收端，无业务逻辑，几乎不需要维护
- 所有 AI 调用、命令生成逻辑全在 exe 端（你的技术栈 TS/JS）
- 收费/账号验证也在 exe 端，Mod 本身免费开源

## 变现

- exe 内置激活码或登录系统
- 每次 AI 调用扣用户余额（充值制）
- 定价：0.05~0.1 元/次，Qwen 成本约 0.01 元，差价可观
- Mod 开源（吸引玩家信任）、exe 闭源（收费逻辑）

## 场景分化

### 场景 A：单人世界

```
用户：在单人模式启动游戏
exe → 检测 .minecraft/saves/{world}/datapacks 目录
     → 生成/更新 datapack/data/namespace/functions/...mcfunction
     → 用 stdout 日志告知用户："/reload 来重新加载"
```

用户手动 `/reload` 或重启世界后，新函数生效。

**优点：** 简单，无需 Mod  
**缺点：** 需要玩家手动 reload

### 场景 B：多人服务器（通过 Mod）

```
用户：连接到装有 Mod 的服务器
exe → POST /setblock { "pos": [x,y,z], "cmd": "/say hello" } 到 localhost:25580
Mod → 用 /setblock 放下命令方块（带 NBT data 中的 Command 字段）
     → 或直接 ClientCommandManager.sendCommand(cmd)
```

**优点：** 无需手动 reload，即时生效  
**缺点：** 需要 Mod，需要 OP 权限

## Mod 实现细节（Kotlin，Fabric 1.21.x）

最小 Mod 需要：

```kotlin
// 用 sun.net.httpserver.HttpServer 嵌入 HTTP 服务器
// 监听 POST /setblock 或 /command
//   /command: { "cmd": "/give @s ..." }  → 直接执行
//   /setblock: { "pos": [x,y,z], "cmd": "..." } → 放命令方块

// 调用 MinecraftClient.getInstance().player?.sendCommand(cmd)
// 检测 OP: player.hasPermissionLevel(2)
```

依赖项：只需 Fabric API。不需要额外网络库（标准库 HttpServer 足够）。

## exe 实现细节（Electron + 现有 TS 代码）

```
src/
  main.ts          # Electron 主进程
  renderer/
    App.vue        # 对话框 UI（现有组件改造）
  logic/
    builder.ts     # 现有，直接复用
    ai-bridge.ts   # 新增：Qwen API 调用 + prompt 工程
    mod-bridge.ts  # 新增：POST 到 localhost:25580
  billing/
    auth.ts        # 激活码 / 账号验证
    usage.ts       # 调用计数
```

## 风险

- **Mod 审核**：如果上 Modrinth / CurseForge，嵌入 HTTP 服务器可能被标记为安全风险，需在 README 解释清楚
- **localhost 冲突**：端口 25580 可能被占用，需支持配置
- **OP 限制**：在非 OP 服务器上功能受限（give 可以，setblock command_block 不行），需在 UI 告知用户
- **AI 幻觉**：Qwen 可能生成语法错误的命令，builder.ts 要能校验并拒绝

## 支持指令清单（1.20.5+）

### 优先级 P1（第一阶段）

| 指令 | 用途 | 复杂度 | 说明 |
|-----|------|--------|------|
| `/give` | 给玩家物品 | 中 | ✅ 已完成 |
| `/setblock` | 单个方块设置 | 中 | 可与命令方块结合 |
| `/say` | 广播消息 | 低 | 纯字符串 |
| `/summon` | 召唤实体 | 中 | NBT 数据结构 |
| `/effect` | 状态效果 | 低 | 效果类型 + 等级 + 时长 |
| `/tp` | 传送玩家 | 低 | 坐标 + 旋转角 |

### 优先级 P2（第二阶段）

| 指令 | 用途 | 复杂度 | 说明 |
|-----|------|--------|------|
| `/scoreboard` | 记分板操作 | 高 | 创建、修改、查询，组件最多 |
| `/fill` | 填充区域 | 中 | 方块范围 + 替换模式 |
| `/clone` | 克隆结构 | 中 | 源目标坐标范围 |
| `/enchant` | 附魔物品 | 低 | 手持物品的附魔 |
| `/attribute` | 属性修改 | 中 | 实体属性基础值修改 |
| `/execute` | 条件执行 | 高 | 复杂的条件和子命令链 |

### 优先级 P3（后续）

| 指令 | 用途 | 复杂度 | 说明 |
|-----|------|--------|------|
| `/loot` | 战利品表 | 高 | 替代品来源 |
| `/recipe` | 配方解锁 | 低 | give/take |
| `/function` | 函数文件 | 高 | 需要文件系统支持 |
| `/clear` | 清空背包 | 中 | 选择器 + 物品数量条件 |

---

## 【下一步：P1 指令探针】（当前任务）

### Context

builder.ts 当前只产出 `/give`，对应 `probes.mjs` 65 条 give 探针。
要扩展到多指令前，先用 mc-verifier **实证 P1 全部指令的语法**（先有真值，再写 builder）。

**核心洞察（已确认）：1.20.5+ 两套序列化**
- `/give` → **item 组件** 语法 `item[comp=val,...]`（最难，已完成）
- `/setblock`（方块实体 NBT）+ `/summon`（实体 NBT）→ **SNBT `{...}`** 语法
- 关键：NBT 里嵌套的物品用**同一套** `{id,count,components:{...}}` 结构
- ⇒ 只要写一次 SNBT 序列化器 + 一次 item-in-NBT 序列化器，setblock/summon（及以后所有带 NBT 的指令）全部复用

因此探针分两类：
1. **give 探针**：与 builder 输出对照（PASS/FAIL，现有逻辑）
2. **新指令探针**（setblock/summon/say/effect/tp）：纯语法调查（valid/invalid/unknown），builder 尚未产出，先建真值表

### 新增探针清单（约 26 条）

复用现有探针对象结构：`{ feature, id, command, builderFamilies, note }`。
RCON 无玩家时：`No player was found` / `Gave` = 语法合法；`Unknown/Expected/<--[HERE]` = 非法。
坐标统一用相对坐标 `~ ~ ~` 避免越界（无加载区块也只判定语法，不影响）。

**say**（families: 全部）
- `say_basic` — `say hello world`
- `say_selector` — `say @a`

**tp / teleport**（全部）
- `tp_coords_abs` — `tp @s 0 64 0`
- `tp_coords_rel` — `tp @s ~ ~ ~`
- `tp_coords_local` — `tp @s ^ ^ ^1`
- `tp_rotation` — `tp @s 0 64 0 90 45`
- `tp_facing` — `tp @s 0 64 0 facing 10 70 10`
- `tp_to_entity` — `tp @s @e[type=pig,limit=1]`
- `teleport_alias` — `teleport @s 0 64 0`

**effect**（全部）
- `effect_give_basic` — `effect give @a minecraft:speed`
- `effect_give_seconds` — `effect give @a minecraft:speed 30`
- `effect_give_amplifier` — `effect give @a minecraft:speed 30 2`
- `effect_give_hide` — `effect give @a minecraft:speed 30 2 true`
- `effect_give_infinite` — `effect give @a minecraft:speed infinite 1`
- `effect_clear_all` — `effect clear @a`
- `effect_clear_one` — `effect clear @a minecraft:speed`

**setblock**（全部）— 方块状态 + 方块实体 NBT
- `setblock_basic` — `setblock ~ ~ ~ minecraft:stone`
- `setblock_blockstate` — `setblock ~ ~ ~ minecraft:oak_log[axis=x]`
- `setblock_mode_keep` — `setblock ~ ~ ~ minecraft:stone keep`
- `setblock_nbt_commandblock` — `setblock ~ ~ ~ minecraft:command_block[facing=up]{Command:"say hi",auto:1b}`
- `setblock_nbt_container` — `setblock ~ ~ ~ minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:1}]}` ★item-in-NBT（小写 count）
- `setblock_nbt_container_components` — `…components:{"minecraft:custom_name":'{"text":"x"}'}…` ★完整 item-in-NBT（含组件）

**summon**（全部）— 实体 NBT
- `summon_basic` — `summon minecraft:pig`
- `summon_coords` / `summon_relative` — `summon minecraft:pig 0 64 0` / `~ ~ ~`
- `summon_nbt_flags` — `summon minecraft:zombie ~ ~ ~ {NoAI:1b,Silent:1b,PersistenceRequired:1b}`
- `summon_nbt_customname_snbt` — `…{CustomName:'{"text":"Boss"}'}`（families: early/legacy/mid，文本走 SNBT 字符串）
- `summon_nbt_customname_json` — `…{CustomName:{"text":"Boss"}}`（families: moder­n，文本走原始 JSON）★与 give 文本同源
- `summon_nbt_handitems` — `…{HandItems:[{id:"minecraft:diamond_sword",count:1},{}]}` ★item-in-NBT 复用
- `summon_nbt_passenger` — `…{Passengers:[{id:"minecraft:chicken"}]}`

**待实证的版本敏感点**（故意写多变体让服务器裁决）：
- 实体属性 NBT：`{attributes:[{id:"minecraft:max_health",base:40}]}` vs 旧 `{Attributes:[{Name:…,Base:…}]}`
- 实体效果 NBT：`active_effects` vs 旧 `ActiveEffects`
- modern CustomName 是否接受裸 JSON（vs 必须 SNBT 字符串）

这些跨版本差异正是 mc-verifier 要回答的——多写 2-3 条变体探针，跑一遍即得真值。

### 文件改动

- **修改** `scripts/mc-verifier/probes.mjs`
  - 在 PROBES 数组追加上述 ~26 条（give 探针保持不动）
  - `classifyResponse` 无需改动（语法判定关键词通用）
- **修改** `scripts/mc-verifier/report.mjs`
  - 现有 give 的 PASS/FAIL 对照逻辑保留
  - 新增：按命令名分组（取 `probe.command` 首词），对非 give 指令输出**语法调查表**（每条 valid/invalid/unknown × 各版本），不做 builder 对照
- **不改** `index.mjs`（探针循环已通用）、`builder.ts`、任何 Vue/catalog

### 验证方式

```bash
# 单版本快速验证（覆盖 SNBT 与组件两套语法）
node scripts/mc-verifier/index.mjs 1.20.6 1.21.5
cat scripts/mc-verifier/results/1.21.5/report.json
# 期望：give 段 FAIL=0；新指令段每条给出 valid/invalid，
#       据此确认 setblock/summon 的 SNBT 与 item-in-NBT 真值
```

跨版本过夜批量：`node scripts/mc-verifier/index.mjs 1.20.5 1.20.6 1.21 1.21.4 1.21.5`（解出版本敏感差异）。

---

## 当前阶段状态

- [x] builder.ts 多版本支持（early/legacy/mid/modern 四族）
- [x] mc-verifier 服务器实证（give，所有版本 FAIL=0）
- [ ] **P1 指令探针（当前任务）：say/tp/effect/setblock/summon 语法实证**
- [ ] 指令架构重构（拆分 give.ts / setblock.ts / ...，共享 SNBT 序列化器）
- [ ] exe 框架（Electron 骨架）
- [ ] Mod 骨架（Fabric，Kotlin，HTTP 监听）
- [ ] AI bridge（Qwen prompt 工程）
- [ ] 计费系统

## 下一步

优先做：**exe 的 Electron 骨架 + Mod 的 HTTP 监听骨架**，验证 exe→Mod 通信链路通（这是最大的未知量）。

---

## 原：MC verifier 工具说明（已完成）

### Context（原）

当前项目 builder.ts 的语法规则来自 CLAUDE.md 文档和模型记忆，无法保证跨版本正确率。
用户需要能在无人值守情况下（跑一晚上）完成多个 Minecraft 版本 `/give` 命令语法的自动验证与适配。

核心洞察：Minecraft 服务器对 `/give` 命令**先做语法解析，再找玩家**。
没有玩家在线时：
- 语法错误 → "Unknown argument..." / "Expected ..."（解析失败）
- 语法正确 → "No player was found" 或 "Given ... to ..."（解析成功）

因此，不需要任何玩家加入服务器，即可通过 RCON 精确判断每条命令的语法合法性。

## 架构

```
scripts/mc-verifier/
├── index.mjs          # 主入口：接收版本列表，串行执行每个版本
├── mojang.mjs         # 从 Mojang 版本清单 API 下载 server.jar
├── server.mjs         # 管理服务器进程生命周期（启动 / 等待就绪 / 关闭）
├── rcon.mjs           # 纯 Node.js RCON 客户端（无外部依赖）
├── probes.mjs         # 每个语法特性的探针命令集
└── report.mjs         # 汇总结果，输出 JSON 报告
```

结果写入：
```
scripts/mc-verifier/results/{version}/
├── raw.json           # 每条探针命令的原始响应
└── report.json        # 已知语法规则的通过/失败汇总
```

## 关键实现细节

### 1. 下载 server.jar（`mojang.mjs`）

```
GET https://launchermeta.mojang.com/mc/game/version_manifest_v2.json
→ 找到目标版本的 url
GET {version_url}
→ 取 downloads.server.url
→ 下载到 scripts/mc-verifier/cache/{version}/server.jar
```

缓存已下载的 JAR，避免重复下载。

### 2. 服务器启动（`server.mjs`）

工作目录：`/tmp/mc-verify-{version}/`

server.properties 最小配置：
```properties
online-mode=false
enable-rcon=true
rcon.port=25575
rcon.password=verify123
max-players=0
spawn-protection=0
enable-command-block=false
```

eula.txt：`eula=true`

启动命令：
```
java -Xmx512M -Xms512M -jar {server.jar} nogui
```

等待 stdout 出现 `Done (` 字符串（通常 30-60 秒），然后连接 RCON。

关闭：RCON 发送 `stop` 命令，等待进程退出。

### 3. RCON 客户端（`rcon.mjs`）

协议简单，用 Node.js net.Socket 实现，无需 npm 依赖：

```
Packet 格式（小端序）：
  [4字节 length] [4字节 request_id] [4字节 type] [payload bytes] [0x00 0x00]
  
type: 3=登录, 2=命令, 0=响应
```

发送命令 → 等待响应（超时 5s）→ 返回响应字符串。

### 4. 探针命令集（`probes.mjs`）

每条探针包含：
- `id`：特性标识符（如 `item_name_snbt`）
- `command`：发送给服务器的完整命令
- `expect`：`"valid"` | `"invalid"`（期望语法是否合法）
- `note`：说明该探针验证什么

探针示例（Java 1.21）：
```javascript
{ id: "basic", command: "give @a minecraft:stone 1", expect: "valid" },
{ id: "custom_name_snbt", command: "give @a minecraft:stone[custom_name='[{\"text\":\"test\"}]'] 1", expect: "valid" },
{ id: "item_name_snbt",   command: "give @a minecraft:stone[item_name='[{\"text\":\"test\"}]'] 1",   expect: "valid" },
{ id: "enchants_levels",  command: "give @a minecraft:stone[enchantments={levels:{unbreaking:1}}] 1", expect: "valid" },
{ id: "enchants_flat",    command: "give @a minecraft:stone[enchantments={unbreaking:1}}] 1",          expect: "invalid" },
// ... 覆盖所有 builder.ts 输出的组件格式
```

判断逻辑：
- 响应包含 `No player was found` 或 `Given` → 语法合法
- 响应包含 `Unknown argument` / `Expected` / `Incorrect argument` → 语法非法
- 其他 → 标记为 `unknown`，人工复查

### 5. 主流程（`index.mjs`）

```
对每个版本：
  1. 下载 / 使用缓存 server.jar
  2. 创建临时目录，写 server.properties + eula.txt
  3. 启动服务器（subprocess），等待 Done
  4. RCON 连接
  5. 逐条发送探针命令，收集结果
  6. RCON 发 stop，等待进程退出
  7. 清理临时目录
  8. 写 results/{version}/raw.json 和 report.json
```

用法：
```bash
node scripts/mc-verifier/index.mjs 1.21 1.21.1 1.21.4
```

### 6. 与 builder.ts 的集成

验证完成后，生成的 `report.json` 作为人工审查的基础：
- `PASS`：当前 builder.ts 输出与服务器期望一致，无需改动
- `FAIL`：builder.ts 输出语法被服务器拒绝，需要修正
- `UNKNOWN`：响应无法判断，需人工抽查

不自动写回 builder.ts（语法规则改动应经过人工确认），但报告中会列出具体差异和建议修改点。

## 需要修改的文件

- **新增** `scripts/mc-verifier/index.mjs`
- **新增** `scripts/mc-verifier/mojang.mjs`
- **新增** `scripts/mc-verifier/server.mjs`
- **新增** `scripts/mc-verifier/rcon.mjs`
- **新增** `scripts/mc-verifier/probes.mjs`
- **新增** `scripts/mc-verifier/report.mjs`
- **修改** `package.json`：添加 `"verify-syntax": "node scripts/mc-verifier/index.mjs"` 脚本
- **修改** `.gitignore`：排除 `scripts/mc-verifier/cache/` 和 `scripts/mc-verifier/results/`（可选保留结果）

不修改 `builder.ts`、`App.vue`、`catalog.ts` 或任何 Vue 组件。

## 前提条件

- Java 17+ 可用（`java -version`）
- 网络可访问 `launchermeta.mojang.com`（Mojang API）
- 磁盘空间：每个版本 server.jar 约 50-200MB

## 验证方式

1. 单版本快速测试：
   ```bash
   node scripts/mc-verifier/index.mjs 1.21.1
   # 预期：~60-90 秒后输出 results/1.21.1/report.json
   ```

2. 检查报告：
   ```bash
   cat scripts/mc-verifier/results/1.21.1/report.json
   # 对照 builder.ts 现有输出，确认 PASS/FAIL 分布
   ```

3. 多版本批量（过夜）：
   ```bash
   node scripts/mc-verifier/index.mjs 1.21 1.21.1 1.21.2 1.21.3 1.21.4
   ```

## 局限与风险

- 服务器启动耗时（每版本 30-90s），5 个版本约需 10-15 分钟，可接受
- Mojang API 限速极低（几乎没有），下载可靠
- 极个别版本 server.jar 启动行为可能有差异（如 1.21.2 以前某些版本的 RCON 初始化时机），需测试后微调等待逻辑
- 不覆盖 Bedrock 版本（无官方 Linux 服务器 JAR）
