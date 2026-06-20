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
];

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

    for (const probe of SEMANTIC_PROBES) {
      log(`\n--- [${probe.id}] ---`);
      log(`    ${probe.note}`);
      const record = { id: probe.id, note: probe.note, setup: [], query: null, cleanup: null };

      for (const cmd of probe.setup) {
        const res = await rcon.send(cmd);
        log(`  setup> ${cmd.slice(0, 120)}`);
        log(`  resp:  ${res.trim().slice(0, 200)}`);
        record.setup.push({ cmd, response: res.trim() });
      }

      await sleep(300); // 等服务器处理 summon/setblock

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

  const outPath = path.join(outDir, "semantic.json");
  fs.writeFileSync(outPath, JSON.stringify({ version, results }, null, 2));
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
