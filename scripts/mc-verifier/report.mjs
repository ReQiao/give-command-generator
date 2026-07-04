/**
 * 把单个版本的探针原始结果汇总成结构化报告。
 *
 * 报告关注两件事：
 *  1. 该版本对每个特性的每个候选格式是否合法（语法真相）。
 *  2. builder.ts 当前对该版本族实际输出的格式是否被服务器接受（PASS/FAIL）。
 */

import { PROBES, familyOf } from "./probes.mjs";

/**
 * @param {string} version
 * @param {Array<{id, feature, command, builderFamilies, result, response}>} results
 */
export function buildReport(version, results) {
  const family = familyOf(version);
  const byId = new Map(results.map((r) => [r.id, r]));

  const features = {};
  for (const probe of PROBES) {
    const res = byId.get(probe.id);
    if (!res) continue;
    if (!features[probe.feature]) {
      features[probe.feature] = { candidates: {}, builderChoice: null, builderResult: null, verdict: null };
    }
    features[probe.feature].candidates[probe.id] = {
      result: res.result,
      builderEmits: probe.builderFamilies.includes(family),
      command: probe.command,
    };
  }

  // 为每个特性确定 builder 在该版本族实际选用的候选，并据此判 verdict
  let pass = 0;
  let fail = 0;
  let unknown = 0;
  for (const [feature, info] of Object.entries(features)) {
    const builderProbes = PROBES.filter(
      (p) => p.feature === feature && p.builderFamilies.includes(family),
    );
    if (builderProbes.length === 0) {
      // builder 在该族不输出此特性 —— 仅作信息记录，不计入 PASS/FAIL
      info.verdict = "N/A";
      continue;
    }
    // 该族 builder 选用的候选结果（取第一个；同特性多候选时综合判定）
    const builderResults = builderProbes.map((p) => byId.get(p.id)?.result ?? "unknown");
    info.builderChoice = builderProbes.map((p) => p.id);
    info.builderResult = builderResults;

    if (builderResults.every((r) => r === "valid")) {
      info.verdict = "PASS";
      pass++;
    } else if (builderResults.some((r) => r === "invalid")) {
      info.verdict = "FAIL";
      fail++;
    } else {
      info.verdict = "UNKNOWN";
      unknown++;
    }
  }

  return {
    version,
    family,
    generatedAt: new Date().toISOString(),
    summary: { pass, fail, unknown, totalFeatures: Object.keys(features).length },
    features,
  };
}

/** 生成简短的人类可读摘要文本 */
export function formatReportText(report) {
  const lines = [];
  lines.push(`# ${report.version}  (builder 族: ${report.family})`);
  lines.push(`摘要: PASS=${report.summary.pass}  FAIL=${report.summary.fail}  UNKNOWN=${report.summary.unknown}`);
  lines.push("");

  const fails = [];
  const infos = [];
  for (const [feature, info] of Object.entries(report.features)) {
    const cand = Object.entries(info.candidates)
      .map(([id, c]) => `${id}=${c.result}${c.builderEmits ? "*" : ""}`)
      .join("  ");
    const line = `[${info.verdict ?? "?"}] ${feature}: ${cand}`;
    if (info.verdict === "FAIL") fails.push(line);
    else infos.push(line);
  }

  if (fails.length) {
    lines.push("## 需修正（builder 输出被服务器拒绝）");
    lines.push(...fails);
    lines.push("");
  }
  lines.push("## 全部特性（* 表示该版本族 builder 实际输出）");
  lines.push(...infos);
  return lines.join("\n");
}
