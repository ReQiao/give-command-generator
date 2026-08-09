#!/usr/bin/env node
/**
 * 语义探针：summon / setblock → data get，研究服务器实际存储的 NBT 结构。
 *
 * 关键原理：
 *   - 语法层（probes.mjs）只验证 Brigadier 能否解析命令。
 *   - 语义层（此脚本）验证服务器实际保留了什么：未知键被静默丢弃，
 *     data get 读回的才是真实存储格式。
 *
 * RCON 无执行者位置，所有坐标用显式绝对值：
 *   superflat 出生区块（0,0）永久已加载，使用 X=0..9, Y=100, Z=0 作为探针坐标。
 *
 * 用法：
 *   node scripts/mc-verifier/semantic-probe.mjs 1.21.5
 *   node scripts/mc-verifier/semantic-probe.mjs 1.20.6 1.21.5
 *
 * 输出：scripts/mc-verifier/results/{version}/semantic.json
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { ensureServerJar } from "./mojang.mjs";
import { startServer } from "./server.mjs";
import { RconClient } from "./rcon.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CACHE_DIR = path.join(__dirname, "cache");
const RESULTS_DIR = path.join(__dirname, "results");

const log = (msg) => console.log(msg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 各探针使用固定绝对坐标，Y=100 保证在 superflat 地表以上。
// 实体 summon 在 (X, 100, 0)；方块 setblock 在 (X, 100, 0)。
const SEMANTIC_PROBES = [
  // =========================================================
  // A. 实体通用字段（读回确认键名大小写）
  // =========================================================
  {
    id: "A1_entity_common",
    note: "zombie 顶层 NBT：确认 CustomName/NoAI/Silent/PersistenceRequired 等字段名",
    setup: [
      `summon minecraft:zombie 0 100 0 {CustomName:'{"text":"Boss"}',NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:["sem_A1"]}`,
    ],
    query: `data get entity @e[tag=sem_A1,limit=1]`,
    cleanup: `kill @e[tag=sem_A1]`,
  },

  // =========================================================
  // B. 属性（attributes vs Attributes）
  // =========================================================
  {
    id: "B1_attr_new_key",
    note: "新格式 attributes[]/id/base → 服务器存储的键名是什么？",
    setup: [
      `summon minecraft:zombie 1 100 0 {attributes:[{id:"minecraft:max_health",base:40.0}],Tags:["sem_B1"]}`,
    ],
    // data get entity 可以加路径，只读指定字段
    query: `data get entity @e[tag=sem_B1,limit=1] Attributes`,
    cleanup: `kill @e[tag=sem_B1]`,
  },
  {
    id: "B2_attr_old_key",
    note: "旧格式 Attributes[]/Name/Base → 服务器存储的键名是什么？",
    setup: [
      `summon minecraft:zombie 2 100 0 {Attributes:[{Name:"minecraft:generic.max_health",Base:40.0}],Tags:["sem_B2"]}`,
    ],
    query: `data get entity @e[tag=sem_B2,limit=1] Attributes`,
    cleanup: `kill @e[tag=sem_B2]`,
  },

  // =========================================================
  // C. 状态效果（active_effects vs ActiveEffects）
  // =========================================================
  {
    id: "C1_effects_new_key",
    note: "新格式 active_effects[]/id(string)/duration/amplifier",
    setup: [
      `summon minecraft:zombie 3 100 0 {active_effects:[{id:"minecraft:speed",duration:200,amplifier:1,show_particles:0b}],Tags:["sem_C1"]}`,
    ],
    query: `data get entity @e[tag=sem_C1,limit=1] active_effects`,
    cleanup: `kill @e[tag=sem_C1]`,
  },
  {
    id: "C2_effects_old_key",
    note: "旧格式 ActiveEffects[]/Id(int)/Duration",
    setup: [
      `summon minecraft:zombie 4 100 0 {ActiveEffects:[{Id:1b,Duration:200,Amplifier:1b}],Tags:["sem_C2"]}`,
    ],
    query: `data get entity @e[tag=sem_C2,limit=1] active_effects`,
    cleanup: `kill @e[tag=sem_C2]`,
  },

  // =========================================================
  // D. HandItems / ArmorItems：item-in-NBT 结构
  // =========================================================
  {
    id: "D1_hand_items",
    note: "HandItems 中 item：id/count(小写?) + components 是否保留",
    setup: [
      `summon minecraft:zombie 5 100 0 {HandItems:[{id:"minecraft:diamond_sword",count:1,components:{"minecraft:enchantment_glint_override":true}},{}],Tags:["sem_D1"]}`,
    ],
    query: `data get entity @e[tag=sem_D1,limit=1] HandItems`,
    cleanup: `kill @e[tag=sem_D1]`,
  },

  // =========================================================
  // E. CustomName：SNBT 字符串 vs 裸 JSON（modern 候选）
  // =========================================================
  {
    id: "E1_custom_name_snbt",
    note: "CustomName 用 SNBT 字符串 '{\"text\":\"...\"}'，data get 读回实际存储类型",
    setup: [
      `summon minecraft:pig 6 100 0 {CustomName:'{"text":"SNBT_NAME","color":"red"}',Tags:["sem_E1"]}`,
    ],
    query: `data get entity @e[tag=sem_E1,limit=1] CustomName`,
    cleanup: `kill @e[tag=sem_E1]`,
  },
  {
    id: "E2_custom_name_raw_json",
    note: "CustomName 用裸 JSON compound {\"text\":\"...\"}（modern 候选），看是否被接受",
    setup: [
      `summon minecraft:pig 7 100 0 {CustomName:{"text":"RAW_JSON","color":"blue"},Tags:["sem_E2"]}`,
    ],
    query: `data get entity @e[tag=sem_E2,limit=1] CustomName`,
    cleanup: `kill @e[tag=sem_E2]`,
  },

  // =========================================================
  // F. 方块实体：命令方块
  // =========================================================
  {
    id: "F1_commandblock",
    note: "命令方块 Command/auto/TrackOutput 字段名大小写",
    setup: [
      `setblock 0 100 1 minecraft:command_block[facing=up]{Command:"say hello from setblock",auto:1b,TrackOutput:0b}`,
    ],
    query: `data get block 0 100 1`,
    cleanup: `setblock 0 100 1 minecraft:air`,
  },

  // =========================================================
  // G. 方块实体：箱子 Items（item-in-NBT 核心）
  // =========================================================
  {
    id: "G1_chest_basic",
    note: "Chest Items[]: Slot/id/count（lowercase count？）基础验证",
    setup: [
      `setblock 1 100 1 minecraft:chest{Items:[{Slot:0b,id:"minecraft:diamond",count:5}]}`,
    ],
    query: `data get block 1 100 1`,
    cleanup: `setblock 1 100 1 minecraft:air`,
  },
  {
    id: "G2_chest_with_components",
    note: "Chest Items[] 含 components：看 components 是否保留、键名格式",
    setup: [
      `setblock 2 100 1 minecraft:chest{Items:[{Slot:0b,id:"minecraft:stone",count:1,components:{"minecraft:enchantment_glint_override":true,"minecraft:custom_name":'{"text":"TestItem"}'}}]}`,
    ],
    query: `data get block 2 100 1`,
    cleanup: `setblock 2 100 1 minecraft:air`,
  },
  {
    id: "G3_chest_old_Count",
    note: "旧键 Count（大写 C）→ 看服务器是否转换为 count",
    setup: [
      `setblock 3 100 1 minecraft:chest{Items:[{Slot:0b,id:"minecraft:gold_ingot",Count:3b}]}`,
    ],
    query: `data get block 3 100 1`,
    cleanup: `setblock 3 100 1 minecraft:air`,
  },

  // =========================================================
  // H. 方块实体：告示牌（1.20+ front_text/back_text）
  // =========================================================
  {
    id: "H1_sign_modern",
    note: "告示牌 front_text/back_text/is_waxed 结构",
    setup: [
      `setblock 4 100 1 minecraft:oak_sign{front_text:{messages:['{"text":"Line1"}','{"text":"Line2"}','{"text":""}','{"text":""}'],color:"red",has_glowing_text:0b},back_text:{messages:['{"text":""}','{"text":""}','{"text":""}','{"text":""}'],color:"black",has_glowing_text:0b},is_waxed:0b}`,
    ],
    query: `data get block 4 100 1`,
    cleanup: `setblock 4 100 1 minecraft:air`,
  },

  // =========================================================
  // I. Passengers（嵌套实体结构）
  // =========================================================
  {
    id: "I1_passengers",
    note: "Passengers[] 嵌套实体：id 字段（entity type id）",
    setup: [
      `summon minecraft:boat 8 100 0 {Type:"oak",Passengers:[{id:"minecraft:chicken",NoAI:1b}],Tags:["sem_I1"]}`,
    ],
    query: `data get entity @e[tag=sem_I1,limit=1] Passengers`,
    cleanup: `kill @e[tag=sem_I1]`,
  },

  // =========================================================
  // J. 数字类型：byte/short/int/float/double
  // =========================================================
  {
    id: "J1_number_types",
    note: "Fire(short) / TicksFrozen(int) / 布尔 1b 等数字类型在 data get 中的表示",
    setup: [
      `summon minecraft:pig 9 100 0 {Fire:200s,TicksFrozen:100,Invulnerable:1b,Tags:["sem_J1"]}`,
    ],
    query: `data get entity @e[tag=sem_J1,limit=1]`,
    cleanup: `kill @e[tag=sem_J1]`,
  },

  // =========================================================
  // K. 「落地检测」组合技真值（AI 机制指南的依据）
  //
  // 爆炸箭 / 地雷 / 落地触发陷阱这类需求，原版没有直接对应的物品或指令，
  // 只能靠「实体落地状态 + 选择器 nbt 过滤 + 循环命令方块」组合实现。
  // 要把这套做法写进 AI 的 prompt，键名与选择器写法必须先实测为真。
  //
  // 关键坑（本组探针实测所得）：箭与掉落物用的是**两个不同的键**——
  //   箭插地   → inGround:1b（此时 OnGround 仍为 0b，用 OnGround 会永远匹配不到）
  //   掉落物落地 → OnGround:1b（掉落物根本没有 inGround 字段）
  // 这正是 AI 最容易张冠李戴的地方，故两者都留了正反面探针。
  //
  // 坐标：超平坦地表 y=-60（草方块），故在 y=-58 落生，1 秒内即可落地，
  // 比从 y=100 自由落体再等 9 秒更快也更稳。
  // =========================================================
  {
    id: "K1_arrow_inground",
    note: "箭插在地上：确认 inGround 键名/类型，并确认此时 OnGround 仍为 0b",
    setup: [`summon minecraft:arrow 10 -58 0 {Tags:["sem_K1"]}`],
    settleMs: 1500,
    query: `data get entity @e[tag=sem_K1,limit=1]`,
    cleanup: `kill @e[tag=sem_K1]`,
  },
  {
    id: "K2_arrow_selector_inground",
    note: "选择器按 nbt={inGround:1b} 过滤已插地的箭 —— 期望 Test passed",
    setup: [`summon minecraft:arrow 11 -58 0 {Tags:["sem_K2"]}`],
    settleMs: 1500,
    // execute if entity 不带 run 时会回 "Test passed. Count: N" / "Test failed"，
    // 可直接经 RCON 读到判定结果；用 run say 则不会回显，无法判读。
    query: `execute if entity @e[type=minecraft:arrow,tag=sem_K2,nbt={inGround:1b}]`,
    cleanup: `kill @e[tag=sem_K2]`,
  },
  {
    id: "K2b_arrow_selector_onground_trap",
    note: "反面：对插地的箭用 nbt={OnGround:1b} 过滤 —— 期望 Test failed（AI 常犯的张冠李戴）",
    setup: [`summon minecraft:arrow 12 -58 0 {Tags:["sem_K2b"]}`],
    settleMs: 1500,
    query: `execute if entity @e[type=minecraft:arrow,tag=sem_K2b,nbt={OnGround:1b}]`,
    cleanup: `kill @e[tag=sem_K2b]`,
  },
  {
    id: "K2c_arrow_inflight_excluded",
    note: "反面：飞行中的箭不应被 inGround:1b 命中 —— 期望 Test failed（否则一射出就触发）",
    setup: [`summon minecraft:arrow 13 100 0 {Tags:["sem_K2c"]}`],
    settleMs: 1200,
    query: `execute if entity @e[type=minecraft:arrow,tag=sem_K2c,nbt={inGround:1b}]`,
    cleanup: `kill @e[tag=sem_K2c]`,
  },
  {
    id: "K3_item_onground",
    note: "掉落物落地：确认 OnGround 键名与 Item 子 compound 结构（Item 大写、id/count 小写）",
    setup: [`summon minecraft:item 14 -58 0 {Item:{id:"minecraft:tnt",count:1},Tags:["sem_K3"]}`],
    settleMs: 1500,
    query: `data get entity @e[tag=sem_K3,limit=1]`,
    cleanup: `kill @e[tag=sem_K3]`,
  },
  {
    id: "K4_item_selector_onground",
    note: "选择器同时按 OnGround 与 Item.id 过滤掉落物 —— 期望 Test passed",
    setup: [`summon minecraft:item 15 -58 0 {Item:{id:"minecraft:tnt",count:1},Tags:["sem_K4"]}`],
    settleMs: 1500,
    query: `execute if entity @e[type=minecraft:item,tag=sem_K4,nbt={OnGround:1b,Item:{id:"minecraft:tnt"}}]`,
    cleanup: `kill @e[tag=sem_K4]`,
  },
  {
    id: "K4b_item_selector_wrong_id",
    note: "反面：Item.id 不匹配时不应命中 —— 期望 Test failed（确认过滤真的按物品区分）",
    setup: [`summon minecraft:item 16 -58 0 {Item:{id:"minecraft:tnt",count:1},Tags:["sem_K4b"]}`],
    settleMs: 1500,
    query: `execute if entity @e[type=minecraft:item,tag=sem_K4b,nbt={OnGround:1b,Item:{id:"minecraft:diamond"}}]`,
    cleanup: `kill @e[tag=sem_K4b]`,
  },
  {
    id: "K5_tnt_fuse",
    note: "summon 出的 TNT 引信键名与类型（fuse 小写 short）",
    setup: [`summon minecraft:tnt 17 -58 0 {fuse:200s,Tags:["sem_K5"]}`],
    query: `data get entity @e[tag=sem_K5,limit=1]`,
    cleanup: `kill @e[tag=sem_K5]`,
  },
  {
    id: "K6_explosive_arrow_chain",
    note: "完整爆炸箭链路：在插地箭的位置生成 TNT —— 期望 Summoned new Primed TNT",
    setup: [`summon minecraft:arrow 18 -58 0 {Tags:["sem_K6"]}`],
    settleMs: 1500,
    query: `execute at @e[type=minecraft:arrow,tag=sem_K6,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:200s}`,
    cleanup: `kill @e[tag=sem_K6]`,
  },
  {
    id: "K7_command_block_roundtrip",
    note: "循环命令方块能否原样承载这类嵌套引号命令（一键部署的 datapack 走同一批命令）",
    setup: [
      `setblock 20 -58 0 minecraft:repeating_command_block{Command:"execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",auto:1b}`,
    ],
    query: `data get block 20 -58 0 Command`,
    cleanup: `setblock 20 -58 0 minecraft:air`,
  },
  {
    id: "K8_fired_arrow_carries_custom_data",
    note:
      "结论已确认（本会话自动化 RCON 在 26.2 上遇到栅栏乱序读回全空，" +
      "改由用户在真实客户端手测确认，见 results/26.2/semantic.json 的" +
      "K8 conclusion 字段）：给箭物品加 custom_data 组件后发射出去，" +
      "落地的箭实体的 item.components 里保留了这份 custom_data，且额外" +
      "镜像出一份顶层 data:{...} 字段，可以直接用选择器 " +
      "nbt={data:{...}} 匹配，不用钻进 item.components 内部。" +
      "这条探针留着给下次环境正常时复跑用自动化结果交叉验证。",
    setup: [
      "setblock 25 -58 0 minecraft:dispenser[facing=up]",
      `item replace block 25 -58 0 container.0 with minecraft:arrow[custom_data={soul_tnt_arrow:1b}]`,
      "setblock 25 -59 0 minecraft:redstone_block",
    ],
    settleMs: 1200,
    query: `data get entity @e[type=minecraft:arrow,limit=1,sort=nearest,x=25,y=-58,z=0]`,
    cleanup: `kill @e[type=minecraft:arrow]`,
  },
];

/** 会话级前置命令：不 forceload 的话，出生区块可能不 tick，实体落不下去也查不到。 */
const SESSION_SETUP = ["forceload add 0 0"];

/**
 * 按 --only=<前缀,前缀...> 过滤要跑的探针（不传则跑全部）。
 * 只想复验某一组时很有用，例如 --only=K 只跑落地检测那组。
 */
function selectProbes() {
  const arg = process.argv.slice(2).find((a) => a.startsWith("--only="));
  if (!arg) return SEMANTIC_PROBES;
  const prefixes = arg.slice("--only=".length).split(",").map((s) => s.trim()).filter(Boolean);
  if (!prefixes.length) return SEMANTIC_PROBES;
  return SEMANTIC_PROBES.filter((p) => prefixes.some((prefix) => p.id.startsWith(prefix)));
}

async function connectWithRetry(rcon, attempts = 6, delayMs = 1500) {
  let lastErr;
  for (let i = 0; i < attempts; i++) {
    try { await rcon.connect(); return; } catch (err) { lastErr = err; }
    await sleep(delayMs);
  }
  throw new Error(`RCON 多次重试仍无法连接：${lastErr?.message}`);
}

async function runVersion(version) {
  log(`\n========== 语义探针  版本 ${version} ==========`);
  const outDir = path.join(RESULTS_DIR, version);
  fs.mkdirSync(outDir, { recursive: true });

  const jarPath = await ensureServerJar(version, CACHE_DIR, log);
  log(`  启动服务器 ...`);
  const server = await startServer({ jarPath, version, log });
  log(`  服务器就绪，连接 RCON ...`);

  const results = [];
  let rcon;
  try {
    rcon = new RconClient(server.rcon);
    await connectWithRetry(rcon);
    log(`  RCON 已连接。等待出生区块加载 ...`);
    await sleep(2000); // 给 spawn chunk 加载时间

    // send() 在超时时会静默返回空串而不是抛错，链路断了也看不出来——
    // 先用 list 冒烟一次，把"整轮探针全是空响应"这种假结果挡在开跑之前。
    const smoke = (await rcon.send("list")).trim();
    if (!smoke) throw new Error("RCON 已连接但 list 无响应，链路不可用（探针结果会全是空值）");
    log(`  RCON 冒烟测试通过：${smoke}`);

    for (const cmd of SESSION_SETUP) {
      log(`  setup(session)> ${cmd} → ${(await rcon.send(cmd)).trim().slice(0, 120)}`);
    }

    for (const probe of selectProbes()) {
      log(`\n--- [${probe.id}] ---`);
      log(`    ${probe.note}`);
      const record = { id: probe.id, note: probe.note, setup: [], query: null, cleanup: null };

      for (const cmd of probe.setup) {
        const res = await rcon.send(cmd);
        log(`  setup> ${cmd.slice(0, 120)}`);
        log(`  resp:  ${res.trim().slice(0, 200)}`);
        record.setup.push({ cmd, response: res.trim() });
      }

      // 默认 300ms 够服务器处理 summon/setblock；需要实体自然下落的探针用 settleMs 延长。
      await sleep(probe.settleMs ?? 300);

      const queryRes = await rcon.send(probe.query);
      log(`  query> ${probe.query}`);
      log(`  DATA:  ${queryRes.trim().slice(0, 800)}`);
      record.query = { cmd: probe.query, response: queryRes.trim() };

      if (probe.cleanup) {
        const cleanRes = await rcon.send(probe.cleanup);
        log(`  clean> ${probe.cleanup.slice(0, 80)} → ${cleanRes.trim().slice(0, 80)}`);
        record.cleanup = { cmd: probe.cleanup, response: cleanRes.trim() };
      }

      await sleep(200);
      results.push(record);
    }
  } finally {
    if (rcon) await rcon.close().catch(() => {});
    log(`\n  关闭服务器 ...`);
    await server.stop();
  }

  // 与已有结果按 id 合并：用 --only 只复跑某一组时，不该抹掉其余探针的历史真值。
  const outPath = path.join(outDir, "semantic.json");
  const merged = new Map();
  if (fs.existsSync(outPath)) {
    try {
      for (const r of JSON.parse(fs.readFileSync(outPath, "utf8")).results ?? []) merged.set(r.id, r);
    } catch {
      log(`  (已有 semantic.json 无法解析，将整体覆盖)`);
    }
  }
  // 空响应代表这一跑没探到（服务器/RCON 抖动），不是"该字段为空"这一事实，
  // 因此不能用它覆盖历史上已经探到的真值——否则一次抖动就把真值抹平。
  let kept = 0;
  for (const r of results) {
    const gotData = Boolean(r.query?.response?.trim());
    if (!gotData && merged.get(r.id)?.query?.response?.trim()) {
      kept++;
      continue;
    }
    merged.set(r.id, r);
  }
  if (kept) log(`  (${kept} 条本次为空响应，已保留此前实测结果)`);
  const ordered = SEMANTIC_PROBES.map((p) => merged.get(p.id)).filter(Boolean);
  fs.writeFileSync(outPath, JSON.stringify({ version, results: ordered }, null, 2));
  log(`\n  语义探针结果已写入 ${outPath}`);
  return results;
}

async function main() {
  const versions = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  if (!versions.length) {
    log("用法: node scripts/mc-verifier/semantic-probe.mjs <版本...>");
    log("例如: node scripts/mc-verifier/semantic-probe.mjs 1.20.6 1.21.5");
    process.exit(1);
  }
  fs.mkdirSync(CACHE_DIR, { recursive: true });
  fs.mkdirSync(RESULTS_DIR, { recursive: true });

  for (const version of versions) {
    try {
      await runVersion(version);
    } catch (err) {
      log(`  !! 版本 ${version} 失败：${err.message}`);
      console.error(err);
    }
  }
}

main().catch((err) => { console.error(err); process.exit(1); });
