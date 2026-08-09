#!/usr/bin/env node
/**
 * 从基岩版 ID 清单生成 src/data/bedrock.generated.ts
 *
 * 为什么需要单独一份：基岩版和 Java 版的 ID 体系并不一致，同一个东西两边名字
 * 可能不同（蜘蛛网 Java 是 cobweb、基岩是 web；南瓜灯 Java 是 jack_o_lantern、
 * 基岩是 lit_pumpkin……本脚本会把这类差异统计出来），而且各有独占条目
 * （基岩的 *_double_slab / *_standing_sign 在 Java 根本不存在）。
 * 在此之前基岩版模式复用的是 Java 目录，导致上面这些物品生成出来的指令在
 * 基岩版里是无效的。
 *
 * 数据来源：scripts/bedrock-id/*.json —— 基岩版脚本 API / 命令补全导出的
 * ID 清单（含官方中文译名）。注意这不是 Mojang 官方数据生成器的产物
 * （基岩版没有 Java 那样的 --reports 生成器），属于第三方导出，可信度不如
 * Java 侧的 registries.json，但目前是能拿到的最完整的基岩 ID 清单。
 * 换新版本数据时直接替换 scripts/bedrock-id/ 下的 json 再跑一次本脚本即可。
 *
 * 用法：node scripts/gen-bedrock-catalog.mjs
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "scripts", "bedrock-id");
const OUT = join(ROOT, "src", "data", "bedrock.generated.ts");

const readJson = (name) => JSON.parse(readFileSync(join(SRC, name), "utf8"));

/** 去掉可能存在的 minecraft: 前缀，统一成裸 path，输出时再补回去。 */
const bare = (id) => id.replace(/^minecraft:/, "");

/**
 * 生成目录行：[完整id, 中文名, 英文名(用于搜索), 分类]。
 * 与 Java 侧 items.generated.ts 的行结构保持一致，这样 mapCatalog / matches /
 * CatalogRow 这些既有工具函数不用为基岩版另写一套。
 */
function build(entries) {
  const seen = new Set();
  const rows = [];
  for (const { name, description } of entries) {
    const path = bare(name);
    // 中文重名会让 mapCatalog 按名字反查时撞车，附加英文 id 消歧（同 Java 侧做法）
    let zh = description || path.replace(/_/g, " ");
    if (seen.has(zh)) zh = `${zh}(${path})`;
    seen.add(zh);
    rows.push([`minecraft:${path}`, zh, path.replace(/_/g, " ")]);
  }
  rows.sort((a, b) => a[0].localeCompare(b[0]));
  return rows;
}

const serialize = (rows) => rows.map((r) => `  ${JSON.stringify(r)},`).join("\n");

const items = build(readJson("item.json").content);
const blocks = build(readJson("block.json").content.blockStateValues);
const entities = build(readJson("entity.json").content);

const banner =
  `// 本文件由 scripts/gen-bedrock-catalog.mjs 自动生成，请勿手工编辑。\n` +
  `// 数据来源：scripts/bedrock-id/*.json（基岩版 ID 清单，含官方中文译名）\n` +
  `// 重新生成：node scripts/gen-bedrock-catalog.mjs\n` +
  `//\n` +
  `// 基岩版 ID 和 Java 版并不通用，务必不要拿 Java 的 ITEMS/BLOCKS 去拼基岩指令。\n\n`;

writeFileSync(
  OUT,
  banner +
    `export const BEDROCK_ITEMS = [\n${serialize(items)}\n] as const;\n\n` +
    `export const BEDROCK_BLOCKS = [\n${serialize(blocks)}\n] as const;\n\n` +
    `export const BEDROCK_ENTITIES = [\n${serialize(entities)}\n] as const;\n`,
);

console.log(`完成: ${items.length} 物品 / ${blocks.length} 方块 / ${entities.length} 实体 -> ${OUT}`);

// 顺带把和 Java 侧的差异打出来，方便人工核对这次换数据有没有引入意外变化。
try {
  const javaSrc = readFileSync(join(ROOT, "src", "data", "items.generated.ts"), "utf8");
  const grab = (name) => {
    const body = new RegExp(`export const ${name} = \\[([\\s\\S]*?)\\n\\] as const;`).exec(javaSrc)?.[1] ?? "";
    return new Map(
      [...body.matchAll(/^ {2}(\[.*\]),$/gm)].map((m) => {
        const row = JSON.parse(m[1]);
        return [row[1], bare(row[0])];
      }),
    );
  };
  for (const [label, javaMap, bedRows] of [
    ["物品", grab("ITEMS"), items],
    ["方块", grab("BLOCKS"), blocks],
  ]) {
    const clash = bedRows.filter(([id, zh]) => javaMap.has(zh) && javaMap.get(zh) !== bare(id));
    console.log(`  ${label}：${clash.length} 条同中文名但两版 id 不同（这些正是复用 Java 目录时会生成错误指令的）`);
  }
} catch {
  // 差异报告只是辅助信息，读不到 Java 目录不影响生成结果
}
