#!/usr/bin/env node
/**
 * 把整套东西跑在本机，用来录视频。
 *
 * # 这个脚本存在的理由
 *
 * 以前要出一个"不依赖服务器"的版本，做法是提交一条 revert 把源码整个退回旧
 * commit（仓库历史里有两次：`重置源码到 decb636`、`重置源码到 16f9244`）。
 * 那样确实不连服务器了，代价是**界面也跟着退回旧版**——新做的液态玻璃、开场
 * 动画、登录门禁全都看不到，而那些恰恰是最该拍进视频里的东西。
 *
 * 这个脚本换一条路：源码一个字不改，把**服务端搬到本机**跑。
 * 客户端那边本来就留好了两个逃生舱（src-tauri/src/remote.rs）：
 *
 *   SOUL_LANTERN_SERVER_BASE      —— 改成 https://127.0.0.1:<port>
 *   SOUL_LANTERN_PINNED_CERT_FILE —— 指向现场签发的临时证书
 *
 * 服务端那边也早就有一个 `SMS_KIND=log`：验证码不发短信，直接打进日志。
 * 于是注册、登录、找回密码、AI 生成、兑换码，整条链路都能在断网的笔记本上走完，
 * 而界面是当前最新的那一版。
 *
 * # 用法
 *
 *   node scripts/demo-local.mjs              # 真实 AI，需要 AI_API_KEY
 *   node scripts/demo-local.mjs --mock       # 完全离线，AI 回固定内容
 *   node scripts/demo-local.mjs --keep       # 保留上一次的账本（余额/账号不清空）
 *   node scripts/demo-local.mjs --no-app     # 只起服务端，自己去开客户端
 *
 * 服务端二进制不在这个仓库里（它已经拆成独立的私有仓库），按顺序找：
 * SOUL_LANTERN_SERVER_BIN 环境变量 → ../soul-lantern-server/target/… → ./server/target/…
 *
 * # 录制时要知道的三件事
 *
 * 1. **默认每次都是全新账本**，余额、账号、已兑换的码全部清空——这样每条都能
 *    从同一个起点重拍。想接着上一条拍就加 `--keep`。
 * 2. **验证码会在这个终端里高亮打出来**，不用去翻服务器日志。
 * 3. **兑换码也会一起打出来**（用同一个 pepper 现签的，本机服务端认），
 *    可以直接演示兑换。注意 `--keep` 之外的每次运行 pepper 都会重新随机，
 *    上一次打出来的码这一次不认。
 */
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = new Set(process.argv.slice(2));
const MOCK = args.has("--mock");
const KEEP = args.has("--keep");
const NO_APP = args.has("--no-app");

const EXE = process.platform === "win32" ? ".exe" : "";
/** 每次跑都用同一个目录：`--keep` 要靠它把账本留下来。 */
const WORK = join(REPO, ".demo-local");

const c = {
  dim: (s) => `\x1b[2m${s}\x1b[0m`,
  bold: (s) => `\x1b[1m${s}\x1b[0m`,
  green: (s) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s) => `\x1b[33m${s}\x1b[0m`,
  red: (s) => `\x1b[31m${s}\x1b[0m`,
  invert: (s) => `\x1b[7m${s}\x1b[0m`,
};

function die(msg, hint) {
  console.error(`\n${c.red("✗")} ${msg}`);
  if (hint) console.error(c.dim(hint));
  process.exit(1);
}

// ---------------------------------------------------------------- 服务端二进制

function findServerBinary() {
  const fromEnv = process.env.SOUL_LANTERN_SERVER_BIN;
  if (fromEnv) {
    if (existsSync(fromEnv)) return fromEnv;
    die(`SOUL_LANTERN_SERVER_BIN 指向的文件不存在：${fromEnv}`);
  }
  const candidates = [
    // 服务端仓库和这个仓库并排放着的情况
    `../soul-lantern-server/target/debug/soul-lantern-server${EXE}`,
    `../soul-lantern-server/target/release/soul-lantern-server${EXE}`,
    // 拆仓之前的老位置，本地还留着 server/ 的话仍然能用
    `server/target/debug/soul-lantern-server${EXE}`,
    `server/target/release/soul-lantern-server${EXE}`,
  ].map((p) => resolve(REPO, p));
  const found = candidates.find(existsSync);
  if (found) return found;
  die(
    "找不到服务端二进制。",
    `服务端在独立的私有仓库里，先在那边跑一次 cargo build，然后：\n` +
      `  SOUL_LANTERN_SERVER_BIN=<二进制路径> node scripts/demo-local.mjs\n\n` +
      `找过这些位置：\n${candidates.map((p) => "  " + p).join("\n")}`,
  );
}

// ---------------------------------------------------------------- 证书

/**
 * 现场签一张 127.0.0.1 的自签证书。
 *
 * `basicConstraints=critical,CA:FALSE` 这条不能省——少了它 rustls 会把这张
 * 证书当成 CA 证书拿来当叶子证书用，报 `CaUsedAsEndEntity` 直接握手失败。
 * 这个坑在服务端仓库里修过一次，参数是从那次的集成测试里抄来的。
 */
function generateCert(dir) {
  const key = join(dir, "demo.key");
  const crt = join(dir, "demo.crt");
  const r = spawnSync(
    "openssl",
    [
      "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1",
      "-keyout", key, "-out", crt,
      "-days", "30", "-nodes",
      "-subj", "/CN=127.0.0.1",
      "-addext", "subjectAltName=IP:127.0.0.1",
      "-addext", "basicConstraints=critical,CA:FALSE",
      "-addext", "keyUsage=critical,digitalSignature,keyEncipherment",
      "-addext", "extendedKeyUsage=serverAuth",
    ],
    { stdio: "pipe" },
  );
  if (r.error || r.status !== 0) {
    die(
      "openssl 跑失败了。",
      process.platform === "win32"
        ? "Windows 上 Git for Windows 自带 openssl，把 C:\\Program Files\\Git\\usr\\bin 加进 PATH 即可。"
        : String(r.stderr ?? r.error),
    );
  }
  return { key, crt };
}

// ---------------------------------------------------------------- 假的上游

/**
 * `--mock` 时顶替真实大模型。
 *
 * 返回的是服务端期望的那个形状（choices[0].message.content 里再套一层 JSON），
 * 服务端会真的去解析它、真的走一遍 dispatch 构建指令——所以录出来的画面和真
 * AI 那条路径完全一样，只是内容固定。断网也能拍。
 */
function startMockUpstream() {
  return new Promise((ok) => {
    const server = createServer((req, res) => {
      let raw = "";
      req.on("data", (d) => (raw += d));
      req.on("end", () => {
        const asked = (() => {
          try {
            const msgs = JSON.parse(raw).messages ?? [];
            return String(msgs.at(-1)?.content ?? "");
          } catch {
            return "";
          }
        })();
        const intents = [
          {
            command: "give",
            form: {
              item: "diamond_sword",
              count: 1,
              name: asked.slice(0, 20) || "演示用的剑",
              enchantments: [{ id: "sharpness", level: 5 }],
            },
          },
        ];
        const content = JSON.stringify({ intents, explanation: "（离线演示模式，内容是固定的）" });
        const body = JSON.stringify({
          choices: [{ message: { content } }],
          usage: { prompt_tokens: 9000, completion_tokens: 700, total_tokens: 9700 },
        });
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(body);
      });
    });
    server.listen(0, "127.0.0.1", () => ok({ server, port: server.address().port }));
  });
}

function freePort() {
  const r = spawnSync(process.execPath, [
    "-e",
    "const s=require('net').createServer();s.listen(0,'127.0.0.1',()=>{console.log(s.address().port);s.close()})",
  ]);
  return Number(String(r.stdout).trim());
}

// ---------------------------------------------------------------- 主流程

const bin = findServerBinary();

if (!KEEP) rmSync(WORK, { recursive: true, force: true });
mkdirSync(WORK, { recursive: true });

// pepper 决定兑换码的校验位。--keep 时固定下来，否则上一轮打出来的码这一轮就不认了。
const pepperFile = join(WORK, "pepper");
let pepper;
if (KEEP && existsSync(pepperFile)) {
  pepper = (await import("node:fs")).readFileSync(pepperFile, "utf8").trim();
} else {
  pepper = randomBytes(48).toString("base64");
  writeFileSync(pepperFile, pepper);
}

const { crt, key } = generateCert(WORK);
const port = freePort();

let mock = null;
let aiEndpoint = process.env.AI_ENDPOINT ?? "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
let aiKey = process.env.AI_API_KEY ?? "";
if (MOCK) {
  mock = await startMockUpstream();
  aiEndpoint = `http://127.0.0.1:${mock.port}/mock`;
  aiKey = "mock-key-not-real";
} else if (!aiKey) {
  die(
    "没有 AI_API_KEY，真实 AI 跑不起来。",
    "要么把 key 放进环境变量（或仓库根目录的 .env.local，它被 .gitignore 挡着）：\n" +
      "  AI_API_KEY=sk-xxx node scripts/demo-local.mjs\n" +
      "要么用离线假数据录制：\n" +
      "  node scripts/demo-local.mjs --mock",
  );
}

const env = {
  ...process.env,
  TLS_CERT: crt,
  TLS_KEY: key,
  LEDGER_PATH: join(WORK, "ledger.json"),
  BIND_ADDR: `127.0.0.1:${port}`,
  AI_ENDPOINT: aiEndpoint,
  AI_MODEL: process.env.AI_MODEL ?? "qwen-plus",
  AI_API_KEY: aiKey,
  AUTH_PEPPER: pepper,
  // 验证码打进日志，不发真短信——录视频时用不着也不该用真短信通道
  SMS_KIND: "log",
  // 默认 60 秒冷却，重拍时会卡住
  SMS_MIN_INTERVAL_SECS: "0",
  RUST_LOG: process.env.RUST_LOG ?? "info",
};

// 先出几张兑换码，方便演示兑换那一段。这一步不绑端口、不写账本，纯离线算 HMAC。
const licenses = (() => {
  const r = spawnSync(bin, ["--gen-license", "3"], { env, encoding: "utf8" });
  return String(r.stdout ?? "")
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => /^SOUL-/.test(s));
})();

const server = spawn(bin, [], { env, stdio: ["ignore", "pipe", "pipe"] });

let app = null;
let shuttingDown = false;
function shutdown(code = 0) {
  if (shuttingDown) return;
  shuttingDown = true;
  app?.kill();
  server.kill();
  mock?.server.close();
  if (!KEEP) rmSync(WORK, { recursive: true, force: true });
  process.exit(code);
}
process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));
server.on("exit", (code) => {
  if (!shuttingDown) {
    console.error(c.red(`\n服务端退出了（code ${code}）。`));
    shutdown(code ?? 1);
  }
});

/** 把验证码从日志里挑出来单独高亮——录制时不用去翻滚屏的日志。 */
function relay(chunk) {
  for (const line of String(chunk).split("\n")) {
    if (!line.trim()) continue;
    const m = line.match(/验证码是\s*(\d{6})/);
    if (m) {
      console.log(`\n${c.invert(c.bold(`  验证码  ${m[1]}  `))}\n`);
    }
    console.log(c.dim(line));
  }
}
server.stdout.on("data", relay);
server.stderr.on("data", relay);

const base = `https://127.0.0.1:${port}`;
console.log(`
${c.bold("灵魂灯笼 · 本地演示")}

  服务端    ${base}
  AI        ${MOCK ? c.yellow("离线假数据（--mock）") : c.green(env.AI_MODEL + " · 真实调用")}
  账本      ${KEEP ? c.yellow("沿用上一次") : c.green("全新")}   ${join(WORK, "ledger.json")}
  短信      ${c.green("SMS_KIND=log")}，验证码直接打在这个终端里
${licenses.length ? `  兑换码    ${licenses.map((l) => c.green(l)).join("\n            ")}` : ""}

${c.dim("Ctrl+C 关掉。" + (KEEP ? "" : "退出时账本和证书一起删掉，下次从干净状态开始。"))}
`);

if (NO_APP) {
  console.log(c.bold("自己开客户端的话，把这两个环境变量带上：\n"));
  console.log(`  SOUL_LANTERN_SERVER_BASE=${base}`);
  console.log(`  SOUL_LANTERN_PINNED_CERT_FILE=${crt}\n`);
} else {
  app = spawn("npm", ["run", "tauri", "dev"], {
    cwd: REPO,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: { ...process.env, SOUL_LANTERN_SERVER_BASE: base, SOUL_LANTERN_PINNED_CERT_FILE: crt },
  });
  app.on("exit", () => shutdown(0));
}
