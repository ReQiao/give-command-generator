/**
 * 从 Mojang 官方分发渠道下载指定版本的 server.jar。
 * 流程：version_manifest_v2.json -> 版本详情 json -> downloads.server.url
 */

import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

const MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

let manifestCache = null;

async function fetchJson(url) {
  const res = await fetch(url, { headers: { "User-Agent": "give-command-generator-verifier" } });
  if (!res.ok) throw new Error(`请求失败 ${res.status} ${res.statusText}: ${url}`);
  return res.json();
}

export async function loadManifest() {
  if (manifestCache) return manifestCache;
  manifestCache = await fetchJson(MANIFEST_URL);
  return manifestCache;
}

export async function listReleaseVersions() {
  const manifest = await loadManifest();
  return manifest.versions.filter((v) => v.type === "release").map((v) => v.id);
}

/**
 * 解析某个版本的 server jar 下载信息。
 * @param {string} version 例如 "1.21.1"
 * @returns {Promise<{url: string, sha1: string, size: number}>}
 */
export async function resolveServerDownload(version) {
  const manifest = await loadManifest();
  const entry = manifest.versions.find((v) => v.id === version);
  if (!entry) {
    throw new Error(`清单中找不到版本 ${version}（可用 release 版本可通过 listReleaseVersions 查看）`);
  }
  const detail = await fetchJson(entry.url);
  const server = detail.downloads?.server;
  if (!server?.url) {
    throw new Error(`版本 ${version} 没有提供 server.jar 下载（可能过旧）`);
  }
  return { url: server.url, sha1: server.sha1, size: server.size };
}

function sha1OfFile(filePath) {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash("sha1");
    const stream = fs.createReadStream(filePath);
    stream.on("data", (d) => hash.update(d));
    stream.on("end", () => resolve(hash.digest("hex")));
    stream.on("error", reject);
  });
}

/**
 * 下载（带缓存）指定版本的 server.jar，返回本地路径。
 * @param {string} version
 * @param {string} cacheDir
 * @param {(msg: string) => void} log
 */
export async function ensureServerJar(version, cacheDir, log = () => {}) {
  const dir = path.join(cacheDir, version);
  fs.mkdirSync(dir, { recursive: true });
  const jarPath = path.join(dir, "server.jar");

  const { url, sha1, size } = await resolveServerDownload(version);

  if (fs.existsSync(jarPath)) {
    const actual = await sha1OfFile(jarPath);
    if (actual === sha1) {
      log(`  缓存命中 server.jar（sha1 校验通过）`);
      return jarPath;
    }
    log(`  缓存 sha1 不匹配，重新下载`);
  }

  log(`  下载 server.jar ${(size / 1048576).toFixed(1)}MB ...`);
  const res = await fetch(url, { headers: { "User-Agent": "give-command-generator-verifier" } });
  if (!res.ok || !res.body) throw new Error(`下载失败 ${res.status}: ${url}`);
  await pipeline(Readable.fromWeb(res.body), fs.createWriteStream(jarPath));

  const actual = await sha1OfFile(jarPath);
  if (actual !== sha1) {
    throw new Error(`下载完成但 sha1 校验失败：期望 ${sha1}，实际 ${actual}`);
  }
  log(`  下载完成，sha1 校验通过`);
  return jarPath;
}
