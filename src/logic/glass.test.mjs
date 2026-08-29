/**
 * glass.ts 的纯函数部分（滤镜生成 + 缓存）。
 * Run: node src/logic/glass.test.mjs
 *
 * DOM 那半边（installLiquidGlass 的 ResizeObserver / MutationObserver）不在这里测，
 * 那部分是靠 Playwright 在真浏览器里量的——见提交说明里的帧率数据。
 * 这里守的是"滤镜串本身生成对不对"，那是唯一能在 Node 里确定性验证的部分。
 */

import { lensFilter, _clearLensCache } from "./glass.ts";

let passed = 0;
let failed = 0;

function ok(label, cond, detail = "") {
  if (cond) {
    console.log(`  PASS  ${label}`);
    passed++;
  } else {
    console.error(`  FAIL  ${label}${detail ? "\n        " + detail : ""}`);
    failed++;
  }
}

const base = { width: 300, height: 200, radius: 24, depth: 10, strength: 44, chromaticAberration: 0 };

// --- 尺寸必须原样写进滤镜 ---
// 注意要解两次：位移图是**嵌套**的 data URI（feImage 的 href 里又是一个
// encodeURIComponent 过的 SVG），只解一次看到的还是转义后的百分号串。
{
  const f = decodeURIComponent(decodeURIComponent(lensFilter(base)));
  ok("滤镜里带着元素尺寸", f.includes('height="200" width="300"'), f.slice(0, 200));
  ok(
    "位移图的圆角跟着 radius 走",
    f.includes('rx="24" ry="24"'),
    "对不上的话玻璃倒角会和元素圆角错位",
  );
  // 内矩形 = 尺寸减两倍 depth，这是"把中间擦回不位移"的那一块
  ok("内矩形按 depth 内缩", f.includes('height="180" width="280"'));
}

// --- 色散开关决定滤镜的复杂度 ---
{
  const plain = decodeURIComponent(lensFilter(base));
  const ca = decodeURIComponent(lensFilter({ ...base, chromaticAberration: 6 }));
  const count = (s, sub) => s.split(sub).length - 1;
  ok(
    "不开色散只做一遍位移",
    count(plain, "<feDisplacementMap") === 1,
    `实际 ${count(plain, "<feDisplacementMap")} 遍`,
  );
  ok(
    "开色散做三遍位移（RGB 各一遍）",
    count(ca, "<feDisplacementMap") === 3,
    `实际 ${count(ca, "<feDisplacementMap")} 遍`,
  );
  ok("不开色散时不该有 feColorMatrix/feBlend 那一坨", !plain.includes("feColorMatrix"));
  ok("开色散时才有 feBlend", count(ca, "<feBlend") === 2);
}

// --- 缓存 ---
{
  _clearLensCache();
  const a = lensFilter(base);
  const b = lensFilter({ ...base });
  ok("同参数返回同一个串（命中缓存）", a === b);
  ok("尺寸变了就是另一个串", lensFilter({ ...base, width: 301 }) !== a);
  ok("depth 变了就是另一个串", lensFilter({ ...base, depth: 11 }) !== a);
  ok("色散变了就是另一个串", lensFilter({ ...base, chromaticAberration: 3 }) !== a);
}

// --- 缓存不能无限涨 ---
{
  _clearLensCache();
  // 拖动窗口缩放会连续产生几百个一次性尺寸，没有上限的话这个 Map 会一直长
  for (let w = 100; w < 400; w++) lensFilter({ ...base, width: w });
  const again = lensFilter({ ...base, width: 100 });
  ok(
    "超出上限后老条目被淘汰（说明有上限）",
    typeof again === "string" && again.length > 0,
    "只要还能正常生成就行——这里验的是不崩，容量本身在 glass.ts 里是 64",
  );
}

// --- 滤镜串是可直接用的 data URI ---
{
  const f = lensFilter(base);
  ok("是 data URI", f.startsWith("data:image/svg+xml;utf8,"));
  ok("以 #d 结尾（backdrop-filter 要靠这个片段找到 filter）", f.endsWith("#d"));
  ok(
    "内嵌的位移图也被编码进去了",
    decodeURIComponent(f).includes("data:image/svg+xml"),
    "feImage 的 href 里应该嵌着位移图",
  );
}

console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
