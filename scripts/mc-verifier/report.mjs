/**
 * 把单个版本的探针原始结果汇总成结构化报告。
 *
 * 报告关注两类内容：
 *  1. give 指令：对每个特性的每个候选格式是否合法，以及 builder.ts 当前对该
 *     版本族实际输出的格式是否被服务器接受（PASS/FAIL）。
 *  2. 其它 P1 指令（say/tp/effect/setblock/summon）：builder 尚未产出，仅做
 *     语法调查——按指令分组列出每条探针的 valid/invalid/unknown 真值表。
 */

import { PROBES, familyOf } from "./probes.mjs";

/** 取命令首词作为指令名，如 "give @a ..." -> "give"。 */
function commandName(command) {
  return (command || "").trim().split(/\s+/)[0] || "";
}

/**
 * @param {string} version
 * @param {Array<{id, feature, command, builderFamilies, result, response}>} results
 */
export function buildReport(version, results) {
  const family = familyOf(version);
  const byId = new Map(results.map((r) => [r.id, r]));

  // ---- 1. give 指令：特性对照（PASS/FAIL）----
  const features = {};
  for (const probe of PROBES) {
    if (commandName(probe.command) !== "give") continue;
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
      (p) => commandName(p.command) === "give" && p.feature === feature && p.builderFamilies.includes(family),
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

  // ---- 2. 其它指令：语法调查（按指令分组，valid/invalid/unknown）----
  const survey = {};
  let sValid = 0;
  let sInvalid = 0;
  let sUnknown = 0;
  for (const probe of PROBES) {
    const cmd = commandName(probe.command);
    if (cmd === "give") continue;
    const res = byId.get(probe.id);
    if (!res) continue;
    if (!survey[cmd]) survey[cmd] = {};
    survey[cmd][probe.id] = {
      feature: probe.feature,
      result: res.result,
      // 该版本族 builder 拟采用的变体（用于标注 *）
      builderIntends: probe.builderFamilies.includes(family),
      command: probe.command,
      note: probe.note,
    };
    if (res.result === "valid") sValid++;
    else if (res.result === "invalid") sInvalid++;
    else sUnknown++;
  }

  return {
    version,
    family,
    generatedAt: new Date().toISOString(),
    summary: { pass, fail, unknown, totalFeatures: Object.keys(features).length },
    surveySummary: { valid: sValid, invalid: sInvalid, unknown: sUnknown, commands: Object.keys(survey).length },
    features,
    survey,
  };
}

/** 生成简短的人类可读摘要文本 */
export function formatReportText(report) {
  const lines = [];
  lines.push(`# ${report.version}  (builder 族: ${report.family})`);
  lines.push(`give 对照: PASS=${report.summary.pass}  FAIL=${report.summary.fail}  UNKNOWN=${report.summary.unknown}`);
  if (report.surveySummary) {
    const s = report.surveySummary;
    lines.push(`指令调查: valid=${s.valid}  invalid=${s.invalid}  unknown=${s.unknown}  (${s.commands} 个指令)`);
  }
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
    lines.push("## give 需修正（builder 输出被服务器拒绝）");
    lines.push(...fails);
    lines.push("");
  }
  lines.push("## give 全部特性（* 表示该版本族 builder 实际输出）");
  lines.push(...infos);

  // 其它指令语法调查
  if (report.survey && Object.keys(report.survey).length) {
    lines.push("");
    lines.push("## 其它指令语法调查（* 表示该版本族拟采用的变体）");
    for (const [cmd, entries] of Object.entries(report.survey)) {
      lines.push(`### /${cmd}`);
      for (const [id, e] of Object.entries(entries)) {
        lines.push(`  [${e.result}] ${id}${e.builderIntends ? "*" : ""}  ${e.note}`);
      }
    }
  }
  return lines.join("\n");
}
