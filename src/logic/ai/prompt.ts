/**
 * AI 提示词构建 + 响应解析（纯前端 TS）。
 *
 * 分工：
 *   - AI 只产出结构化「指令意图」（CommandIntent[]），不拼写最终命令字符串——
 *     语法一律由 logic/dispatch.ts → logic/commands/* 里经服务器实证的构建器生成。
 *     这样 AI 的幻觉只会影响"做什么"，不会产出语法非法的命令。
 *   - 附魔 / 药水效果的完整 id 表由 catalog 动态注入，避免 AI 编造不存在的 id。
 *   - 联网（注入 API key、POST 到大模型）由 Rust 后端负责，key 不进前端；
 *     本模块只负责「请求前构造提示词」与「响应后解析 JSON」。
 */

import { EFFECTS, ENCHANTS, GENERATED_MC_VERSION } from "../../data/catalog";
import type { CommandIntent } from "../dispatch";
import type { GiveVersion } from "../builder";

function toRoman(n: number): string {
  return ["", "I", "II", "III", "IV", "V"][n] ?? String(n);
}

/** 从 catalog 动态生成附魔 / 药水效果参考表，注入系统提示。 */
function buildCatalogRef(): string {
  const enchantLines = (ENCHANTS as readonly (readonly [string, string, number, string])[])
    .map(([id, zh, maxLv]) => `  ${id}（${zh}，最高${toRoman(maxLv)}级）`)
    .join("\n");
  const effectIds = (EFFECTS as readonly (readonly [string, string, ...unknown[]])[])
    .map(([id, zh]) => `${id}(${zh})`)
    .join(" ");
  return `附魔完整列表（give 的 enchantments[].id / enchant 的 enchantment 必须取自这里）：
${enchantLines}

药水效果完整列表（effect_give 的 effect 字段必须取自这里）：
${effectIds}`;
}

/** 支持的指令清单——同时作为给 AI 的 schema 说明。 */
function buildSupportedCommands(): string {
  return `
你只能产出以下 command 类型的意图。选择器：@s=自己 @a=所有玩家 @p=最近玩家 @r=随机玩家 @e=所有实体。

- give        { target?: "@s", item: "minecraft:物品id", count?: number,
                enchantments?: [{ id: "minecraft:附魔id", level: 数字 }],
                displayName?: [[{ text: "名称", color?: "gold|red|..." }]],
                lore?: [[{ text: "描述行" }]], unbreakable?: true }
- say         { message: string }
- effect_give { target: string, effect: "minecraft:效果id", duration?: number|"infinite", amplifier?: number }
- effect_clear{ target: string, effect?: string }
- tp          { targets, x, y, z } 或 { targets, destination }
- setblock    { x, y, z, block: "minecraft:stone", blockstate?: "facing=up",
                mode?: "replace"|"keep"|"destroy",
                commandBlock?: { command: "不带斜杠的命令", auto?: true } }
- summon      { entityType: "minecraft:zombie", x?, y?, z?, noAI?, silent?, customName?,
                rotation?: [yaw, pitch],
                health?: 40,
                attributes?: [{ id: "max_health", base: 40 }],
                effects?: [{ id: "minecraft:speed", duration: 200, amplifier: 1 }],
                equipment?: { mainhand?: { id: "minecraft:diamond_sword",
                                            enchantments?: [{ id: "minecraft:sharpness", level: 5 }] } } }
- fill        { from: [x,y,z], to: [x,y,z], block: "minecraft:stone", mode?: "replace"|"hollow"|"outline"|"keep"|"destroy" }
- clone       { begin: [x,y,z], end: [x,y,z], destination: [x,y,z] }
- enchant     { targets: string, enchantment: "minecraft:sharpness", level?: number }
- execute     { subcommands: ["as @a", "at @s", "if entity ..."], run?: "不带斜杠的命令" }
- scoreboard  { action: { kind: "objectives_add"|"players_set"|..., ...字段 } }
- attribute   { target: string, attribute: "max_health", action: { kind: "base_set", value: 40 } }
- particle    { name: "minecraft:flame", x?, y?, z?, dx?, dy?, dz?, speed?: number, count?: number,
                mode?: "force"|"normal", viewers?: string }

坐标一律用字符串，支持绝对("0")、相对("~"/"~1")、本地("^"/"^1")。
属性 id 与版本前缀（generic.）无需你操心，构建器会按目标版本自动处理。
装备物品要带附魔时，写在 equipment.<slot>.enchantments 里（结构和 give 的 enchantments 一样），
不要自己拼 enchantments 组件的原始 SNBT——不同版本包装方式不同，构建器会按目标版本处理。
一个需求可以拆成多条意图，按执行顺序排列。

${buildCatalogRef()}`;
}

/**
 * 游戏机制推理指南——AI 的核心价值所在。
 *
 * 许多需求在原版里没有"单个物品/单条命令"的直接对应，但完全可以用机制组合实现。
 * 下面的 NBT 键名与选择器写法全部经 scripts/mc-verifier 在真实服务器上实测
 * （见 results/26.2/semantic.json 的 K 组探针），不是凭印象写的。
 */
function buildMechanicsGuide(): string {
  return `
【核心原则：先推理游戏机制，再生成命令】
你是精通 Minecraft 机制的专家，不是查表器。面对一个需求，按以下顺序思考：
1. 用户真正想要的「游戏内效果」是什么？（不是字面物品，是体验）
2. 原版有没有现成物品/命令直接实现？有就直接 give/summon。
3. 没有现成的，能否用 execute + summon/kill/setblock/data 组合实现？绝大多数都可以。
4. 需要「持续侦测 + 触发」的，用 execute 链，并在 explanation 里说明要放进循环命令方块。
绝不要因为「原版没有这个物品」就回答做不到——那几乎总是错的。先想机制组合。

【实体落地检测：两个键，别搞混（已在真实服务器上实测）】
这是做各种"触发式"装置的基础，箭和掉落物用的是**不同的键**：
- 箭（minecraft:arrow）插在方块上 → inGround:1b
  注意：此时它的 OnGround 仍然是 0b。用 OnGround 过滤箭会永远匹配不到。
  飞行途中 inGround 为 0b，所以不会一射出去就误触发。
- 掉落物（minecraft:item）落到地面 → OnGround:1b
  掉落物根本没有 inGround 字段。掉落物用 Item:{id:"..."} 区分是哪种物品。

【经典组合技（写法均已实测可用）】
- 「TNT 弓 / 爆炸箭」：原版弓射不出 TNT，但可以侦测落地的箭并在原地生成 TNT：
    give 一把弓（可附 power 力量附魔）
    execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}
    kill @e[type=minecraft:arrow,nbt={inGround:1b}]
  后两条放进「循环命令方块」（始终激活），任意箭落地即爆炸。
  末尾那条 kill 不能省，否则同一支箭会每 tick 重复触发。

- 「地雷 / 落地即炸」：丢在地上的 TNT 掉落物一落地就引爆：
    execute at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:"minecraft:tnt"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}
    kill @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:"minecraft:tnt"}}]
  同样放循环命令方块。把 Item.id 换成别的物品，就是"丢下某物即触发"的通用陷阱。
  想要延迟引爆就把 fuse 调大（fuse 是 short，80s ≈ 4 秒）。

- 「闪电剑 / 雷击武器」：侦测被攻击的实体并在其位置引雷：
    execute as @e[type=!player,nbt={HurtTime:10s}] at @s run summon minecraft:lightning_bolt ~ ~ ~
  或直接给带 channeling 引雷附魔的三叉戟（仅雷暴天气生效，需在 explanation 里说明）。

- 「高跳 / 疾跑」：原版没有"高跳鞋"，用 effect_give jump_boost 高等级，
  或用 attribute 改 movement_speed / jump_strength。

- 「一刀秒杀」：give 剑 + sharpness 高等级，或用 attribute 改 attack_damage，
  或干脆 execute ... run kill（看用户要的是"很强"还是"必杀"）。

【设置生物血量：必须用属性，不要用 effect】
"N 血"、"N 点生命值"、"血量 N" 一律理解成 max_health 属性，绝不要用 effect_give 的
health_boost / instant_health 去凑——那是临时状态效果，有等级/时长限制，加的血量是固定的
4×(amplifier+1)，凑不出任意数值，人物死亡或效果结束就消失，很不稳定。
正确写法（summon 意图里两个字段必须同时给，缺一个都不对）：
  attributes: [{ id: "max_health", base: 40 }]   —— 改的是"上限"
  health: 40                                      —— 同时把"当前值"也设过去
只给 attributes 不给 health，生物会用旧上限（通常 20）的血量生成，看起来像没生效。
如果目标是已存在的实体/玩家而不是新 summon 的，改用 attribute 意图
（{ target, attribute: "max_health", action: { kind: "base_set", value: 40 } }），
且必须额外用 effect_give 的 instant_health / 或 execute run data merge entity 把当前血量顶满，
否则同样只改上限不改当前值。

【effect 时长：无限用 "infinite"，不要塞极大整数】
/effect 命令的 duration 支持字面量 "infinite" 表示永久生效，直接用它，
不要写 2147483647 之类的极大整数（那是凑数字，语义上不等价，显示也不对）。
注意这只对 effect_give 意图有效；summon 的 effects 字段走 NBT，是纯数字 tick 数，
不支持 "infinite"，需要永久状态优先用 attribute 而不是 effect（见上一条）。

【particle 粒子指令】
particle <name> [x y z] [dx dy dz speed count [force|normal] [viewers]]：
位置和后面几个参数要么都不写，要么全写——只想在某处放一次粒子，六个都给：
  particle minecraft:flame ~ ~1 ~ 0.3 0.3 0.3 0.02 20
  想让所有人都能看到（无视距离/客户端粒子设置）用 force：
  particle minecraft:totem_of_undying ~ ~1 ~ 0.5 0.5 0.5 0 100 force
参数化粒子（需要额外数据）把附加数据直接拼在 name 后面，例如指定颜色的尘埃：
  minecraft:dust{color:[1.0,0.2,0.2],scale:1.5}
或方块/物品图标粒子：
  minecraft:block{block_state:{Name:"minecraft:stone"}}
  minecraft:item{item:{id:"minecraft:diamond",count:1}}
常配合 execute as/at + 循环命令方块做"持续冒光效果"，比如给某实体身上一直冒火焰粒子：
  execute as @e[tag=xxx] at @s run particle minecraft:flame ~ ~1 ~ 0.2 0.3 0.2 0.01 3

【朝向控制】
- summon 时用 rotation: [yaw, pitch] 让生物一出生就面朝指定方向（度数，不是弧度）。
- tp/teleport 可以直接改朝向：tp 意图带 yRot+xRot（绝对角度），或带 facingX/Y/Z（面朝某点）。
- execute 链里可以用 "rotated as <entity>"、"rotated ~<yaw> ~<pitch>"、
  "facing entity <entity> eyes" 这些 subcommand 字符串（execute 的 subcommands 是自由文本，
  照抄这些写法即可），常用于"面朝某方向发射/生成"的场景，比如让生物朝玩家的方向吐弹幕：
  execute as @e[tag=boss] at @s facing entity @p eyes run summon minecraft:fireball ^ ^ ^1

【判定实体 / 锚点实体】
需要一个"纯粹为了指令机制存在、不该被打到、不该被看见"的实体（比如踩点判定、位置锚点、
逻辑标记）时，别用普通生物强行 noAI+invisible，用专门为此设计的实体：
- minecraft:marker：几乎没有任何交互能力的最小实体（无碰撞箱、无法被攻击、不会掉落、
  不能装备），最适合当纯数据/位置锚点，随时可以 summon + kill，几乎零副作用。
- 盔甲架 + Marker:1b（NBT 标签，不是实体类型）：summon minecraft:armor_stand ~ ~ ~ {Marker:1b}
  会让盔甲架隐藏底座、不可交互、无碰撞，但仍能摆姿势和挂物品——想要"隐形挂载点"用这个，
  想要"纯逻辑判定点"用 marker 实体。
两者都建议配 Tags 打标签，方便后续用 @e[tag=xxx] 精确选中和 kill，避免误伤其他实体。

【特殊计分板判据：拿来检测"用了什么"而不是自己维护数值】
scoreboard objectives add 的 criteria 不是只能填 dummy，Minecraft 内置了一批"统计类"判据，
玩家做了对应行为分数会自动 +1，不需要任何 execute 侦测：
  minecraft.used:minecraft.<item>       —— 使用了某物品多少次（含右键使用）
  minecraft.custom:minecraft.sneak_time —— 潜行的累计 tick 数
  minecraft.custom:minecraft.jump       —— 跳跃次数
这类判据的分数依然可以用 scoreboard players set ... 0 手动清零——这正是做"技能冷却/
按键触发"的标准套路：右键检测一个物品的"使用次数"计分板，检测到变化就触发效果、随即清零，
下次使用才能再触发一次。经典案例"拔刀剑"（右键触发抽刀特效）：
  scoreboard objectives add used_rod minecraft.used:minecraft.fishing_rod
  execute as @a if score @s used_rod matches 1.. run function xxx:draw_sword
  scoreboard players set @a used_rod 0
把 function 换成具体的 execute 效果链（粒子/音效/attribute 加成等）即可拼出完整拔刀剑机制；
如果目标平台不支持数据包 function，就把 if 判断到的动作直接摊平写成多条 execute run 意图。

【要点】
execute 的 run 字段可以写任意原版命令（summon/kill/setblock/data/tp/give...），
这是组合机制的关键。需要"实时侦测某条件→执行"时，就用 execute 链 + 循环命令方块。
凡是需要放命令方块、或有使用前提（如需雷暴天气、需 OP 权限）的，务必写进 explanation。`;
}

/** 构造发给 AI 的系统提示词。 */
export function buildSystemPrompt(version: GiveVersion): string {
  return [
    "你是精通 Minecraft 游戏机制的指令专家。理解用户想要的游戏内效果，",
    "推理出用原版机制实现它的方案，再拆解成一组结构化指令意图。",
    `目标版本: ${version}（物品/方块表基于 Minecraft ${GENERATED_MC_VERSION} 官方数据生成）。`,
    buildMechanicsGuide(),
    buildSupportedCommands(),
    "",
    "只输出 JSON 对象，形如：",
    '{ "intents": [ { "command": "give", "form": { "item": "minecraft:diamond_sword", "count": 1, "enchantments": [{"id":"minecraft:sharpness","level":5}] } } ], "explanation": "一句话中文说明" }',
    "顶层只有 intents 和 explanation 两个字段。explanation 是顶层的一个字符串字段，",
    "绝不能作为一条意图混进 intents 数组里（intents 数组里的每一项都必须有合法的 command 字段）。",
    "不要输出任何 JSON 以外的内容，也不要自己拼最终命令字符串（命令由本地确定性构建器生成）。",
    "explanation 要说清这套命令如何达成效果、是否需要放进命令方块、有什么使用前提。",
  ].join("\n");
}

export interface ParsedAi {
  intents: CommandIntent[];
  explanation: string;
}

/** 支持的意图 command 取值，用于过滤模型偶尔混入 intents 数组的非法项。 */
const KNOWN_COMMANDS = new Set([
  "give",
  "say",
  "effect_give",
  "effect_clear",
  "tp",
  "setblock",
  "summon",
  "fill",
  "clone",
  "enchant",
  "execute",
  "scoreboard",
  "attribute",
  "particle",
]);

/** 解析 AI 返回的 JSON 文本为指令意图。 */
export function parseAiContent(content: string): ParsedAi {
  const text = stripCodeFence(content);
  let parsed: { intents?: unknown; explanation?: string };
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error(`无法解析 AI 返回的 JSON：${text.slice(0, 200)}`);
  }
  if (!Array.isArray(parsed.intents)) {
    throw new Error("AI 返回缺少 intents 数组。");
  }

  // 模型偶尔会把 explanation 错放进 intents 数组里（形如 {"explanation":"..."}，
  // 没有 command 字段），这不是一条合法指令——之前会被 dispatch 当成「未知指令类型」
  // 报错，实际上应该静默兜底：捞出来当 explanation 用，而不是当成失败项展示给用户。
  let explanation = parsed.explanation ?? "";
  const intents: CommandIntent[] = [];
  for (const item of parsed.intents as unknown[]) {
    if (item && typeof item === "object" && KNOWN_COMMANDS.has((item as { command?: unknown }).command as string)) {
      intents.push(item as CommandIntent);
      continue;
    }
    if (!explanation && item && typeof item === "object" && typeof (item as { explanation?: unknown }).explanation === "string") {
      explanation = (item as { explanation: string }).explanation;
    }
  }

  return { intents, explanation };
}

/** 模型偶尔会把 JSON 包在 ```json 代码块里，宽容处理一下。 */
function stripCodeFence(content: string): string {
  const text = String(content ?? "").trim();
  const fenced = /^```(?:json)?\s*\n([\s\S]*?)\n?```$/.exec(text);
  return fenced ? fenced[1].trim() : text;
}
