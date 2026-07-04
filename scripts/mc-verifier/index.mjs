#!/usr/bin/env node
/**
 * Minecraft give 命令语法自动验证器。
 *
 * 用法：
 *   node scripts/mc-verifier/index.mjs 1.21 1.21.1 1.21.4
 *   node scripts/mc-verifier/index.mjs --list          # 列出可用 release 版本
 *
 * 对每个版本：下载 server.jar -> 启动服务器 -> 通过 RCON 逐条发送探针 ->
 * 分类响应 -> 写出 raw.json 与 report.json。
 *
 * 全程无需玩家加入：服务器对 give 命令先解析语法再查找目标玩家，
 * "No player was found" 即代表语法合法。
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { ensureServerJar, listReleaseVersions } from "./mojang.mjs";
import { startServer } from "./server.mjs";
import { RconClient } from "./rcon.mjs";
import { PROBES, classifyResponse } from "./probes.mjs";
import { buildReport, formatReportText } from "./report.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CACHE_DIR = path.join(__dirname, "cache");
const RESULTS_DIR = path.join(__dirname, "results");

const log = (msg) => console.log(msg);

async function runVersion(version) {
  log(`\n=== 版本 ${version} ===`);
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
    // RCON 端口可能稍晚于 "Done" 才可连，做几次重试
    await connectWithRetry(rcon, 6, 1500);
    log(`  RCON 已连接，发送 ${PROBES.length} 条探针 ...`);

    for (const probe of PROBES) {
      let response = "";
      let result = "unknown";
      try {
        response = await rcon.send(probe.command);
        result = classifyResponse(response);
      } catch (err) {
        response = `[ERROR] ${err.message}`;
        result = "error";
      }
      results.push({
        id: probe.id,
        feature: probe.feature,
        command: probe.command,
        builderFamilies: probe.builderFamilies,
        result,
        response: response.trim(),
      });
      log(`    ${result.padEnd(7)} ${probe.id}`);
    }
  } finally {
    if (rcon) await rcon.close().catch(() => {});
    log(`  关闭服务器 ...`);
    await server.stop();
  }

  fs.writeFileSync(path.join(outDir, "raw.json"), JSON.stringify({ version, results }, null, 2));
  const report = buildReport(version, results);
  fs.writeFileSync(path.join(outDir, "report.json"), JSON.stringify(report, null, 2));
  const text = formatReportText(report);
  fs.writeFileSync(path.join(outDir, "report.txt"), text + "\n");

  log(`\n${text}\n`);
  log(`  结果已写入 ${outDir}`);
  return report;
}

async function connectWithRetry(rcon, attempts, delayMs) {
  let lastErr;
  for (let i = 0; i < attempts; i++) {
    try {
      await rcon.connect();
      return;
    } catch (err) {
      lastErr = err;
      await new Promise((r) => setTimeout(r, delayMs));
    }
  }
  throw new Error(`RCON 多次重试仍无法连接：${lastErr?.message}`);
}

async function main() {
  const args = process.argv.slice(2);

  if (args.includes("--list")) {
    const versions = await listReleaseVersions();
    log(versions.slice(0, 60).join("  "));
    log(`\n共 ${versions.length} 个 release 版本（仅显示前 60 个）`);
    return;
  }

  const versions = args.filter((a) => !a.startsWith("--"));
  if (versions.length === 0) {
    log("用法: node scripts/mc-verifier/index.mjs <版本...>  例如 1.21 1.21.1 1.21.4");
    log("      node scripts/mc-verifier/index.mjs --list");
    process.exit(1);
  }

  fs.mkdirSync(CACHE_DIR, { recursive: true });
  fs.mkdirSync(RESULTS_DIR, { recursive: true });

  const summaries = [];
  for (const version of versions) {
    try {
      const report = await runVersion(version);
      summaries.push({ version, ...report.summary });
    } catch (err) {
      log(`  !! 版本 ${version} 验证失败：${err.message}`);
      summaries.push({ version, error: err.message });
    }
  }

  log(`\n=== 总汇总 ===`);
  for (const s of summaries) {
    if (s.error) log(`  ${s.version}: 失败 (${s.error})`);
    else log(`  ${s.version}: PASS=${s.pass} FAIL=${s.fail} UNKNOWN=${s.unknown}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
