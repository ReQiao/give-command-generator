// 一次性迁移脚本：把客户端 catalog 数据转换成服务器（Rust）用的静态 (id, 中文名)
// 表。只提取 id/中文名两列——服务器侧只做「AI 编造 id 校验」+「mapCatalog 按名字
// 反查 id」，两者都只用到 CatalogRow 的前两个字段，不需要英文名/分类/等级/描述
// 这些客户端 UI 专用的字段。
//
// 数据本身仍然是从客户端这几份文件复制过来的独立副本（不是共享单一数据源），
// 见迁移计划里"catalog 数据在客户端和服务器各保留一份"的决定。
//
// 用法：node scripts/gen-server-catalog.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, "..");

/** 从 `export const NAME = [...]` 里抠出数组字面量文本，用 Function 当 JSON 解析（数据文件本身就是纯字面量，无副作用）。 */
function extractArray(source, constName) {
  const marker = `export const ${constName} = [`;
  const start = source.indexOf(marker);
  if (start === -1) throw new Error(`找不到 ${constName}`);
  const arrStart = start + marker.length - 1; // 指向 '['
  let depth = 0;
  let i = arrStart;
  for (; i < source.length; i++) {
    if (source[i] === "[") depth++;
    else if (source[i] === "]") {
      depth--;
      if (depth === 0) {
        i++;
        break;
      }
    }
  }
  const literal = source.slice(arrStart, i);
  // eslint-disable-next-line no-new-func
  return new Function(`return ${literal};`)();
}

function rustEscape(s) {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

/** 生成 `pub static NAME: &[(&str, &str)] = &[...]` 形式，只取每行前两列。 */
function toRustPairArray(constName, rows) {
  const lines = rows.map((row) => `    ("${rustEscape(row[0])}", "${rustEscape(row[1])}"),`);
  return `pub static ${constName}: &[(&str, &str)] = &[\n${lines.join("\n")}\n];\n`;
}

const catalogSrc = readFileSync(path.join(root, "src/data/catalog.ts"), "utf8");
const itemsSrc = readFileSync(path.join(root, "src/data/items.generated.ts"), "utf8");
const bedrockSrc = readFileSync(path.join(root, "src/data/bedrock.generated.ts"), "utf8");

const tables = [
  ["ITEMS", extractArray(itemsSrc, "ITEMS")],
  ["BLOCKS", extractArray(itemsSrc, "BLOCKS")],
  ["ENTITIES", extractArray(itemsSrc, "ENTITIES")],
  ["PARTICLES", extractArray(itemsSrc, "PARTICLES")],
  ["BEDROCK_ITEMS", extractArray(bedrockSrc, "BEDROCK_ITEMS")],
  ["BEDROCK_BLOCKS", extractArray(bedrockSrc, "BEDROCK_BLOCKS")],
  ["BEDROCK_ENTITIES", extractArray(bedrockSrc, "BEDROCK_ENTITIES")],
  ["ENCHANTS", extractArray(catalogSrc, "ENCHANTS")],
  ["EFFECTS", extractArray(catalogSrc, "EFFECTS")],
  ["ATTRIBUTES", extractArray(catalogSrc, "ATTRIBUTES")],
];

let out = `// 本文件由 scripts/gen-server-catalog.mjs 自动生成，请勿手工编辑。
// 数据来源：src/data/catalog.ts + items.generated.ts + bedrock.generated.ts 的
// 独立副本（只取 id + 中文名两列，服务器侧校验/mapCatalog 只需要这两列）。
// 重新生成：node scripts/gen-server-catalog.mjs

`;
for (const [name, rows] of tables) {
  out += toRustPairArray(name, rows);
  out += "\n";
}

const outPath = path.join(root, "server/src/give/catalog_data.rs");
writeFileSync(outPath, out, "utf8");

for (const [name, rows] of tables) {
  console.log(`${name}: ${rows.length} 条`);
}
console.log(`写入 ${outPath}`);
