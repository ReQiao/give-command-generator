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
                attributes?: [{ id: "max_health", base: 40 }],
                effects?: [{ id: "minecraft:speed", duration: 200, amplifier: 1 }],
                equipment?: { mainhand?: { id: "minecraft:diamond_sword" } } }
- fill        { from: [x,y,z], to: [x,y,z], block: "minecraft:stone", mode?: "replace"|"hollow"|"outline"|"keep"|"destroy" }
- clone       { begin: [x,y,z], end: [x,y,z], destination: [x,y,z] }
- enchant     { targets: string, enchantment: "minecraft:sharpness", level?: number }
- execute     { subcommands: ["as @a", "at @s", "if entity ..."], run?: "不带斜杠的命令" }
- scoreboard  { action: { kind: "objectives_add"|"players_set"|..., ...字段 } }
- attribute   { target: string, attribute: "max_health", action: { kind: "base_set", value: 40 } }

坐标一律用字符串，支持绝对("0")、相对("~"/"~1")、本地("^"/"^1")。
属性 id 与版本前缀（generic.）无需你操心，构建器会按目标版本自动处理。
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
    "不要输出任何 JSON 以外的内容，也不要自己拼最终命令字符串（命令由本地确定性构建器生成）。",
    "explanation 要说清这套命令如何达成效果、是否需要放进命令方块、有什么使用前提。",
  ].join("\n");
}

export interface ParsedAi {
  intents: CommandIntent[];
  explanation: string;
}

/** 解析 AI 返回的 JSON 文本为指令意图。 */
export function parseAiContent(content: string): ParsedAi {
  const text = stripCodeFence(content);
  let parsed: { intents?: CommandIntent[]; explanation?: string };
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error(`无法解析 AI 返回的 JSON：${text.slice(0, 200)}`);
  }
  if (!Array.isArray(parsed.intents)) {
    throw new Error("AI 返回缺少 intents 数组。");
  }
  return { intents: parsed.intents, explanation: parsed.explanation ?? "" };
}

/** 模型偶尔会把 JSON 包在 ```json 代码块里，宽容处理一下。 */
function stripCodeFence(content: string): string {
  const text = String(content ?? "").trim();
  const fenced = /^```(?:json)?\s*\n([\s\S]*?)\n?```$/.exec(text);
  return fenced ? fenced[1].trim() : text;
}
