/**
 * 管理单个 Minecraft 服务器进程的生命周期：
 * 准备工作目录 -> 启动 -> 等待就绪 -> 关闭 -> 清理。
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const RCON_PORT = 25575;
const RCON_PASSWORD = "verify";

/**
 * 写入运行所需的最小配置（eula + server.properties）。
 * 使用超平坦世界、关闭结构生成以加快启动。
 */
function prepareWorkDir(version) {
  const workDir = fs.mkdtempSync(path.join(os.tmpdir(), `mc-verify-${version}-`));
  fs.writeFileSync(path.join(workDir, "eula.txt"), "eula=true\n");

  const props = [
    "online-mode=false",
    "enable-rcon=true",
    `rcon.port=${RCON_PORT}`,
    `rcon.password=${RCON_PASSWORD}`,
    "broadcast-rcon-to-ops=false",
    "max-players=0",
    "spawn-protection=0",
    "enable-command-block=false",
    // 留空 generator-settings 时 26.x 会打一条
    // "No key layers in MapLike[{}]" 的 ERROR 日志，但世界仍按经典超平坦生成
    // （实测地表草方块在 y=-60），可以忽略。
    // 注意：不要试图用 generator-settings 显式指定 layers 来消掉这条日志——
    // 实测传 JSON 会让区块生成阶段直接把服务器带崩。
    "level-type=minecraft:flat",
    "generate-structures=false",
    "view-distance=3",
    "simulation-distance=3",
    "sync-chunk-writes=false",
    "server-port=25565",
    "motd=verifier",
  ].join("\n");
  fs.writeFileSync(path.join(workDir, "server.properties"), props + "\n");

  return workDir;
}

/**
 * 启动服务器并等待 "Done (" 就绪日志。
 * @param {object} opts
 * @param {string} opts.jarPath
 * @param {string} opts.version
 * @param {number} opts.startupTimeoutMs
 * @param {(msg: string) => void} opts.log
 * @returns {Promise<{workDir, proc, rcon: {port, password}, stop, logTail}>}
 */
export function startServer({ jarPath, version, startupTimeoutMs = 180000, log = () => {} }) {
  const workDir = prepareWorkDir(version);
  // 把 jar 复制进工作目录，避免污染缓存目录
  const localJar = path.join(workDir, "server.jar");
  fs.copyFileSync(jarPath, localJar);

  const proc = spawn(
    "java",
    ["-Xmx1024M", "-Xms512M", "-XX:+UseG1GC", "-jar", "server.jar", "nogui"],
    { cwd: workDir, stdio: ["pipe", "pipe", "pipe"] },
  );

  const logTail = [];
  const pushLog = (line) => {
    logTail.push(line);
    if (logTail.length > 400) logTail.shift();
  };

  return new Promise((resolve, reject) => {
    let ready = false;
    let stdoutBuf = "";

    const timer = setTimeout(() => {
      if (ready) return;
      cleanup();
      proc.kill("SIGKILL");
      reject(new Error(`服务器启动超时（${startupTimeoutMs}ms），最后日志:\n${logTail.slice(-20).join("\n")}`));
    }, startupTimeoutMs);

    const onStdout = (chunk) => {
      stdoutBuf += chunk.toString();
      let idx;
      while ((idx = stdoutBuf.indexOf("\n")) >= 0) {
        const line = stdoutBuf.slice(0, idx);
        stdoutBuf = stdoutBuf.slice(idx + 1);
        pushLog(line);
        if (!ready && /]: Done \(/.test(line)) {
          ready = true;
          clearTimeout(timer);
          // 给 RCON 线程一点初始化时间
          setTimeout(() => {
            resolve({
              workDir,
              proc,
              rcon: { port: RCON_PORT, password: RCON_PASSWORD, host: "127.0.0.1" },
              logTail,
              stop: () => stopServer(proc, workDir, log),
            });
          }, 1500);
        }
      }
    };

    const onStderr = (chunk) => {
      for (const line of chunk.toString().split("\n")) {
        if (line.trim()) pushLog(`[stderr] ${line}`);
      }
    };

    const onExit = (code) => {
      cleanup();
      if (!ready) {
        reject(new Error(`服务器进程在就绪前退出（code=${code}）:\n${logTail.slice(-25).join("\n")}`));
      }
    };

    const cleanup = () => {
      proc.stdout.off("data", onStdout);
      proc.stderr.off("data", onStderr);
      proc.off("exit", onExit);
    };

    proc.stdout.on("data", onStdout);
    proc.stderr.on("data", onStderr);
    proc.once("exit", onExit);
    proc.once("error", (err) => {
      clearTimeout(timer);
      reject(new Error(`无法启动 java 进程：${err.message}`));
    });
  });
}

/**
 * 优雅关闭服务器：stdin 发送 stop，超时则 SIGKILL，最后删除工作目录。
 */
function stopServer(proc, workDir, log = () => {}) {
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      try {
        fs.rmSync(workDir, { recursive: true, force: true });
      } catch (err) {
        log(`  清理工作目录失败：${err.message}`);
      }
      resolve();
    };

    proc.once("exit", finish);

    try {
      proc.stdin.write("stop\n");
    } catch {
      // stdin 可能已关闭，直接走强杀路径
    }

    setTimeout(() => {
      if (!done) {
        try { proc.kill("SIGKILL"); } catch { /* ignore */ }
        setTimeout(finish, 1000);
      }
    }, 15000);
  });
}
