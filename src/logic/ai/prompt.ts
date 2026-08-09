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

import { EFFECTS, ENCHANTS, ENTITIES, GENERATED_MC_VERSION } from "../../data/catalog";
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
  const entityIds = (ENTITIES as readonly (readonly [string, string, ...unknown[]])[])
    .map(([id, zh]) => `${id}(${zh})`)
    .join(" ");
  return `附魔完整列表（give 的 enchantments[].id / enchant 的 enchantment 必须取自这里）：
${enchantLines}

药水效果完整列表（effect_give 的 effect 字段必须取自这里）：
${effectIds}

实体类型完整列表（summon 的 entityType 必须取自这里，本地会校验，编造的一律构建失败）：
${entityIds}`;
}

/** 支持的指令清单——同时作为给 AI 的 schema 说明。 */
function buildSupportedCommands(): string {
  return `
你只能产出以下 command 类型的意图。选择器：@s=自己 @a=所有玩家 @p=最近玩家 @r=随机玩家 @e=所有实体。

- give        { target?: "@s", item: "minecraft:物品id", count?: number,
                enchantments?: [{ id: "minecraft:附魔id", level: 数字 }],
                displayName?: [[{ text: "名称", color?: "gold|red|..." }]],
                lore?: [[{ text: "描述行" }]], unbreakable?: true,
                customData?: "{自定义键:值}" }
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
- execute     { subcommands: ["as @a", "at @s", "if entity ..."], run?: "不带斜杠的命令", loop?: true }
- scoreboard  { action: { kind: "objectives_add"|"players_set"|..., ...字段 } }
- attribute   { target: string, attribute: "max_health", action: { kind: "base_set", value: 40 } }
- particle    { name: "minecraft:flame", x?, y?, z?, dx?, dy?, dz?, speed?: number, count?: number,
                mode?: "force"|"normal", viewers?: string }

坐标一律用字符串，支持绝对("0")、相对("~"/"~1")、本地("^"/"^1")。
属性 id 与版本前缀（generic.）无需你操心，构建器会按目标版本自动处理。
装备物品要带附魔时，写在 equipment.<slot>.enchantments 里（结构和 give 的 enchantments 一样），
不要自己拼 enchantments 组件的原始 SNBT——不同版本包装方式不同，构建器会按目标版本处理。
没有单独的"kill"意图：想表达"杀掉某选择器匹配到的实体"，用 execute 的
{ subcommands: ["as <选择器>"], run: "kill @s" }，run 里也能写任意其他原版命令。
execute 的 loop 字段：这条命令需要"每 tick 持续侦测"（不是执行一次就完事）就设为 true——
系统部署时会自动把它挂进 datapack 的 tick 循环，玩家 /reload 后立刻生效，不需要再手动放
命令方块。绝大多数"侦测式"组合技（见下面 mechanics guide）都要标 loop:true。
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

【用 custom_data 给物品打标记，区分"哪一件"而不是"这一类"（已实测确认）】
很多组合技需要区分"这个特定的物品/箭"和"世界上其他同类物品"——比如一把特制
的弓射出的箭要爆炸，但玩家背包里、骷髅射出的其他普通箭不该受影响。给 give
意图加 customData 字段（放一个自定义 SNBT 复合，比如 "{soul_tnt_arrow:1b}"），
已经在真实服务器上验证过：这份数据会跟着抛射物一起保留在发射后的箭实体里，
并且额外镜像出一份**顶层** data:{...} 字段，可以直接用选择器
nbt={data:{你的键:值}} 匹配，不需要钻进 item.components 内部（那边的键名
带引号和冒号，选择器 nbt= 语法里很难写）。凡是"只有这一件特制物品才该触发
效果，其他同类物品不该被误伤"的需求，都用这个技巧标记 + 过滤，别漏掉这一步
直接对"所有箭/所有掉落物"生效——那样一来除了这把特制弓，任何弓射出的箭、
其他玩家射的箭都会被误伤。

【经典组合技（写法均已实测可用，侦测类命令记得标 loop:true）】
- 「TNT 弓 / 爆炸箭」：原版弓射不出 TNT，需要给特制箭打标记，只让这种箭爆炸，
  不能影响玩家背包里的普通箭或者别人射的箭：
    give 一把弓（普通弓即可，可附 power 力量附魔）
    give 若干支特制箭：{ item: "minecraft:arrow", customData: "{soul_tnt_arrow:1b}" }
    execute（loop:true）: at @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}
    execute（loop:true）: as @e[type=minecraft:arrow,nbt={inGround:1b,data:{soul_tnt_arrow:1b}}] run kill @s
  两条 execute 都标 loop:true，只有插了 customData 标记的特制箭落地才爆炸，
  部署后自动生效。末尾那条 kill 不能省，否则同一支箭会每 tick 重复触发。
  explanation 里要提醒玩家：这些箭要用弓射出去才会触发，直接扔在地上不会。

- 「地雷 / 落地即炸」：丢在地上的 TNT 掉落物一落地就引爆：
    execute（loop:true）: at @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:"minecraft:tnt"}}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}
    execute（loop:true）: as @e[type=minecraft:item,nbt={OnGround:1b,Item:{id:"minecraft:tnt"}}] run kill @s
  把 Item.id 换成别的物品，就是"丢下某物即触发"的通用陷阱。
  想要延迟引爆就把 fuse 调大（fuse 是 short，80s ≈ 4 秒）。

- 「闪电剑 / 雷击武器」：侦测被攻击的实体并在其位置引雷（loop:true）：
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
常配合 execute as/at + loop:true 做"持续冒光效果"，比如给某实体身上一直冒火焰粒子：
  execute（loop:true）: as @e[tag=xxx] at @s run particle minecraft:flame ~ ~1 ~ 0.2 0.3 0.2 0.01 3

【朝向控制】
- summon 时用 rotation: [yaw, pitch] 让生物一出生就面朝指定方向（度数，不是弧度）。
- tp/teleport 可以直接改朝向：tp 意图带 yRot+xRot（绝对角度），或带 facingX/Y/Z（面朝某点）。
- execute 链里可以用 "rotated as <entity>"、"rotated ~<yaw> ~<pitch>"、
  "facing entity <entity> eyes" 这些 subcommand 字符串（execute 的 subcommands 是自由文本，
  照抄这些写法即可），常用于"面朝某方向发射/生成"的场景，比如让生物朝玩家的方向吐弹幕：
  execute as @e[tag=boss] at @s facing entity @p eyes run summon minecraft:fireball ^ ^ ^1

【"正前方 N 度范围内"这类锥形判定：不需要数据包，纯 execute 链就能做】
这类需求不要回答"需要单独写数据包"——那是错的，用 execute 的
anchored + facing + positioned + 距离选择器就能做出来，是社区常用的纯指令锥形判定技巧
（一条 execute 意图，subcommands 按顺序放这些片段，run 放实际效果）：
  subcommands: ["as @a", "at @s", "anchored eyes", "facing entity @e[type=minecraft:zombie,limit=1,sort=nearest] eyes",
                "anchored feet", "positioned ^ ^ ^<forward>", "as @e[type=minecraft:zombie,distance=..<radius>]"]
  run: "<效果，比如 damage @s 5 或 effect give @s minecraft:weakness 1">
原理：先把执行者转向目标方向（facing 只转朝向不挪位置），再把执行位置沿这个朝向"投影"到
正前方 <forward> 格处（positioned ^ ^ ^N 是相对当前朝向前进，不是真的移动实体），最后看
目标是否落在这个投影点的 <radius> 格半径内——run 里的 @s 到这里已经指向命中的目标实体。
弧度越大要求的 radius/forward 比值越大：半锥角 θ 时 radius ≈ forward × tan(θ)。
"正前方 120 度"是半锥角 60°，tan(60°)≈1.73，比如 forward=3 就配 radius≈5.2
（这套判定在角度很大时会变粗略，是近似锥形而不是精确 120°，需要在 explanation 里提醒
用户这是近似）。必须标 loop:true 才能持续侦测。

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
下次使用才能再触发一次。经典案例"拔刀剑"（右键触发抽刀特效），拆成三条意图（前两条标 loop:true）：
  scoreboard: objectives_add，objective=used_rod，criteria=minecraft.used:minecraft.fishing_rod（一次性，不用 loop）
  execute（loop:true）: subcommands=["as @a","if score @s used_rod matches 1.."], run="<抽刀效果，比如 particle/playsound/attribute 加成>"
  execute（loop:true）: subcommands=["as @a","if score @s used_rod matches 1.."], run="scoreboard players set @s used_rod 0"
不要把效果包进一个"function xxx:xxx"去调用——本系统不支持引用自定义 datapack 函数名，
效果必须摊平成具体的 execute run 命令（如上面两条 execute），不能凭空写一个函数路径。

【概率 / 随机效果：不需要数据包，用游戏内时间做伪随机数】
纯指令没有"扔骰子"式的真随机，但可以用 gametime 取模凑一个足够随机的伪随机数，
不需要 predicate/loot table 之类的数据包功能。三条意图（分数字段自己取名即可）：
  scoreboard: objectives_add，objective=rng，criteria=dummy
  execute: subcommands=["store result score @s rng"], run="time query gametime"
  scoreboard: players_operation，targets=@s，objective=rng，operation="%=", source=<常量,比如用一个固定值 100 的计分板>，sourceObjective=const
  execute: subcommands=["as @a","if score @s rng matches 0..49"], run="<50% 概率触发的效果>"
想要固定概率 P%，把 matches 的区间宽度设成 P（比如 0..49 就是 100 取模下的 50%）。
这类判断如果要"持续生效"（比如每次攻击都判定一次），配套的 execute 记得标 loop:true；
如果只是响应某个一次性触发（比如玩家用了个物品才判定一次），就不需要 loop。

【要点】
execute 的 run 字段可以写任意原版命令（summon/kill/setblock/data/tp/give...），
这是组合机制的关键。需要"实时侦测某条件→执行"时，就用 execute 链 + loop:true（见上）。
凡是有使用前提的（如需雷暴天气、需 OP 权限、锥形判定是近似值），务必写进 explanation。`;
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
    '{ "intents": [' +
      '{ "command": "give", "form": { "item": "minecraft:bow", "count": 1, "enchantments": [{"id":"minecraft:power","level":5}] } }, ' +
      '{ "command": "execute", "form": { "subcommands": ["at @e[type=minecraft:arrow,nbt={inGround:1b}]"], "run": "summon minecraft:tnt ~ ~ ~ {fuse:0s}", "loop": true } }' +
      '], "explanation": "一句话中文说明" }',
    "每一条意图都必须是 { command, form } 两个字段——所有参数（包括 subcommands、item、",
    "target 等）都要包在 form 对象里，绝不能直接摊平写在意图对象顶层。这一点尤其容易在",
    "execute 上出错：不要漏掉 form 包装，也不要漏掉 subcommands 数组（哪怕只有一个元素，",
    "也必须是数组形式，不能省略、不能写成字符串）——漏了会导致该条指令完全构建失败。",
    "顶层只有 intents 和 explanation 两个字段。explanation 是顶层的一个字符串字段，",
    "绝不能作为一条意图混进 intents 数组里（intents 数组里的每一项都必须有合法的 command 字段）。",
    "不要输出任何 JSON 以外的内容，也不要自己拼最终命令字符串（命令由本地确定性构建器生成）。",
    "explanation 要说清这套命令如何达成效果、是否需要一键部署才能生效、有什么使用前提。",
    "",
    "【严禁编造 id：本地会做存在性校验，编造的一律构建失败】",
    "give.item、summon.equipment.<slot>.id 必须是本系统 ITEMS 官方物品表里的 id（本提示词",
    "上面机制指南之外没有单独列出全表，是因为条目太多——但你训练数据里的常见原版物品/方块",
    "英文 id 基本都在这张表里，只要不是你自己编的名字就大概率能过）；setblock/fill 的 block",
    "必须是官方方块 id；give.enchantments/summon.effects/effect_give.effect/enchant.enchantment/",
    "attribute.attribute/summon.entityType 必须取自上面已经完整列出的附魔表、药水效果表、",
    "实体类型表，不能超出这几张表——这几张表是完整的，不存在「表外还有但没列出」的情况。",
    "凡是编不出对应中文效果、只能靠编一个看起来像的 id 硬凑的情况，宁可换成机制指南里教的",
    "组合技（execute+summon/attribute等），也不要编造一个不存在的物品/方块/实体/附魔/效果/",
    "属性 id——编造的 id 不会让效果生效，只会导致这条意图直接构建失败，玩家什么都拿不到。",
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
      const rec = item as Record<string, unknown>;
      if (rec.form && typeof rec.form === "object") {
        intents.push(item as CommandIntent);
      } else {
        // 模型偶尔会忘记套 form 包装，把参数直接摊平写在意图对象上
        // （例如 { "command": "execute", "subcommands": [...], "run": "..." }，
        // 没有 form 字段）。这会导致构建器拿到空表单——execute 报"至少需要一个
        // 子命令"、give 直接退回默认物品——看起来像是随机丢参数，其实是同一个
        // 结构性问题。这里兜底：把除 command 外的其余字段收拢成 form。
        const { command, ...rest } = rec;
        intents.push({ command, form: rest } as CommandIntent);
      }
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
