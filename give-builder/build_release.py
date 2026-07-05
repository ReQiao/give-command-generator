#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
give 一键编译 / 发布工具（个人自用，tkinter）

流程：
  1. 在 ~/give-command-generator 里把版本号写成 x.x.0
     （package.json / package-lock.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json）
     然后 git add + commit + git pushall
  2. 在 ~/give-command-generator 里 npm run tauri build（本地出 windows-x64）
     把 msi / nsis 产物改名并放进 ~/give-releases/x.x/windows-x64/
  3. gh 触发远程 workflow give-buildall-other，等它跑完，下载产物，
     按 平台-架构 改名放进 ~/give-releases/x.x/<平台>-<架构>/

依赖：本机已装 node/npm、rust、tauri-cli、gh（已登录），且 git 有 pushall 别名。

远程 workflow 约定（give-buildall-other.yml 需要按这些名字上传 artifact）：
  macos-x64 / macos-silicon / windows-x86 / windows-ARM64 / linux-x64 / linux-ARM64
每个 artifact 里包含该平台对应后缀的文件即可，本脚本按后缀自动改名。
"""

import json
import queue
import re
import shutil
import subprocess
import threading
import time
import tkinter as tk
from pathlib import Path
from tkinter import scrolledtext

# ---------------- 可按需修改的常量 ----------------
HOME = Path.home()
REPO = HOME / "give-command-generator"
BUNDLE = REPO / "src-tauri" / "target" / "release" / "bundle"
WORKFLOW = "give-buildall-other.yml"   # 远程 workflow 文件名
BRANCH = "main"                        # 触发 workflow 用的分支

# 远程各平台 artifact -> [(在 artifact 里按此后缀查找, 输出文件后缀)]
REMOTE = {
    "macos-x64":     [("*.app", "app"), ("*.dmg", "dmg"), ("*.tar.gz", "tar.gz")],
    "macos-silicon": [("*.app", "app"), ("*.dmg", "dmg"), ("*.tar.gz", "tar.gz")],
    "windows-x86":   [("*-setup.exe", "exe"), ("*.msi", "msi")],
    "windows-ARM64": [("*-setup.exe", "exe"), ("*.msi", "msi")],
    "linux-x64":     [("*.AppImage", "AppImage"), ("*.deb", "deb"), ("*.rpm", "rpm")],
    "linux-ARM64":   [("*.AppImage", "AppImage"), ("*.deb", "deb"), ("*.rpm", "rpm")],
}


def releases_dir(short: str) -> Path:
    return HOME / "give-releases" / short


# ---------------- 日志（线程安全） ----------------
LOG_Q: "queue.Queue[str]" = queue.Queue()


def log(msg: str) -> None:
    LOG_Q.put(str(msg))


# ---------------- 子进程封装 ----------------
def sh(cmd: str, cwd: Path | None = None, check: bool = True) -> int:
    """运行命令并把输出实时打到日志。"""
    log(f"$ {cmd}")
    proc = subprocess.Popen(
        cmd, cwd=str(cwd) if cwd else None, shell=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, bufsize=1, encoding="utf-8", errors="replace",
    )
    assert proc.stdout is not None
    for line in proc.stdout:
        log(line.rstrip())
    proc.wait()
    if check and proc.returncode != 0:
        raise RuntimeError(f"命令失败(exit {proc.returncode}): {cmd}")
    return proc.returncode


def sh_capture(cmd: str, cwd: Path | None = None) -> str:
    r = subprocess.run(
        cmd, cwd=str(cwd) if cwd else None, shell=True,
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    return (r.stdout or "").strip()


# ---------------- 文件工具 ----------------
def first_match(root: Path, pattern: str) -> Path | None:
    if not root.exists():
        return None
    hits = sorted(root.rglob(pattern))
    return hits[0] if hits else None


def place(src: Path, target: Path) -> None:
    """移动 src 到 target，已存在则覆盖（支持 .app 目录）。"""
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        if target.is_dir():
            shutil.rmtree(target)
        else:
            target.unlink()
    shutil.move(str(src), str(target))


# ---------------- 步骤 1：改版本号 + 推送 ----------------
def set_json_version(path: Path, full: str) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = full
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    log(f"✓ {path.name} -> {full}")


def set_lock_version(path: Path, full: str) -> None:
    if not path.exists():
        log("⚠ 无 package-lock.json，跳过")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = full
    pkgs = data.get("packages")
    if isinstance(pkgs, dict) and "" in pkgs and isinstance(pkgs[""], dict):
        pkgs[""]["version"] = full
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    log(f"✓ {path.name} -> {full}")


def set_toml_version(path: Path, full: str) -> None:
    text = path.read_text(encoding="utf-8")
    new, n = re.subn(r'(?m)^(version\s*=\s*")[^"]*(")', rf"\g<1>{full}\g<2>", text, count=1)
    if n == 0:
        raise RuntimeError("Cargo.toml 未找到 version 行")
    path.write_text(new, encoding="utf-8")
    log(f"✓ {path.name} -> {full}")


def bump_and_push(short: str, full: str) -> None:
    set_json_version(REPO / "package.json", full)
    set_lock_version(REPO / "package-lock.json", full)
    set_json_version(REPO / "src-tauri" / "tauri.conf.json", full)
    set_toml_version(REPO / "src-tauri" / "Cargo.toml", full)

    sh("git add -A", cwd=REPO)
    rc = sh(f'git commit -m "release v{short}"', cwd=REPO, check=False)
    if rc != 0:
        log("（没有需要提交的改动，跳过 commit）")
    sh("git pushall", cwd=REPO)


# ---------------- 步骤 2：本地 windows-x64 ----------------
def move_local_windows(short: str) -> None:
    dest = releases_dir(short) / "windows-x64"
    msi = first_match(BUNDLE / "msi", "*.msi")
    if msi:
        place(msi, dest / f"give-{short}-windows-x64.msi")
        log(f"✓ give-{short}-windows-x64.msi")
    else:
        log("⚠ 未找到本地 msi 产物")
    exe = first_match(BUNDLE / "nsis", "*-setup.exe")
    if exe:
        place(exe, dest / f"give-{short}-windows-x64.exe")
        log(f"✓ give-{short}-windows-x64.exe")
    else:
        log("⚠ 未找到本地 nsis exe 产物")


# ---------------- 步骤 3：远程 workflow ----------------
def latest_run_id() -> str:
    return sh_capture(
        f'gh run list --workflow={WORKFLOW} -L 1 --json databaseId --jq ".[0].databaseId // empty"',
        cwd=REPO,
    )


def trigger_and_wait() -> str:
    before = latest_run_id()
    sh(f"gh workflow run {WORKFLOW} --ref {BRANCH}", cwd=REPO)
    log("已触发远程 workflow，等待运行出现…")
    run_id = ""
    for _ in range(60):  # 最多等 5 分钟让 run 出现
        time.sleep(5)
        cur = latest_run_id()
        if cur and cur != before:
            run_id = cur
            break
    if not run_id:
        raise RuntimeError("未能获取到新的 workflow 运行 ID")
    log(f"运行 ID: {run_id}，开始监视（每 15s 刷新）…")
    sh(f"gh run watch {run_id} --exit-status --interval 15", cwd=REPO)
    return run_id


def download_and_organize(run_id: str, short: str) -> None:
    tmp = HOME / f"_gh_artifacts_{short}"
    if tmp.exists():
        shutil.rmtree(tmp)
    tmp.mkdir(parents=True)
    sh(f'gh run download {run_id} -D "{tmp}"', cwd=REPO)

    for artifact, specs in REMOTE.items():
        src = tmp / artifact
        if not src.exists():
            log(f"⚠ 缺少产物目录: {artifact}")
            continue
        dest = releases_dir(short) / artifact
        for pattern, ext in specs:
            f = first_match(src, pattern)
            if not f:
                log(f"⚠ {artifact} 未找到 {pattern}")
                continue
            target = dest / f"give-{short}-{artifact}.{ext}"
            place(f, target)
            log(f"✓ {target.name}")

    shutil.rmtree(tmp, ignore_errors=True)


# ---------------- 主流程 ----------------
def worker(short: str, do_bump: bool, do_build: bool, do_remote: bool) -> None:
    full = short + ".0"
    try:
        log(f"=== 开始：版本 {short}（写入 {full}）===")
        if do_bump:
            log("【1】修改版本号并推送")
            bump_and_push(short, full)
        if do_build:
            log("【2】本地编译 windows-x64")
            sh("npm run tauri build", cwd=REPO)
            move_local_windows(short)
        if do_remote:
            log("【3】触发远程 workflow 并下载其它平台")
            run_id = trigger_and_wait()
            download_and_organize(run_id, short)
        log(f"=== 全部完成，产物在 {releases_dir(short)} ===")
    except Exception as e:  # noqa: BLE001 自用工具，兜底打印即可
        log(f"✗ 出错: {e}")
    finally:
        root.after(0, lambda: start_btn.config(state="normal"))


def on_start() -> None:
    short = ver_var.get().strip()
    if not re.fullmatch(r"\d+\.\d+", short):
        log("✗ 版本号格式应为 x.x（例如 4.1）")
        return
    start_btn.config(state="disabled")
    threading.Thread(
        target=worker,
        args=(short, bump_var.get(), build_var.get(), remote_var.get()),
        daemon=True,
    ).start()


# ---------------- UI ----------------
root = tk.Tk()
root.title("give 一键编译")
root.geometry("760x520")

top = tk.Frame(root)
top.pack(fill="x", padx=8, pady=6)

tk.Label(top, text="版本号 (x.x)：").pack(side="left")
ver_var = tk.StringVar()
tk.Entry(top, textvariable=ver_var, width=10).pack(side="left")

bump_var = tk.BooleanVar(value=True)
build_var = tk.BooleanVar(value=True)
remote_var = tk.BooleanVar(value=True)
tk.Checkbutton(top, text="改版本+推送", variable=bump_var).pack(side="left", padx=4)
tk.Checkbutton(top, text="本地编译", variable=build_var).pack(side="left", padx=4)
tk.Checkbutton(top, text="远程编译+下载", variable=remote_var).pack(side="left", padx=4)

start_btn = tk.Button(top, text="开始", width=10, command=on_start)
start_btn.pack(side="right")

logbox = scrolledtext.ScrolledText(root, wrap="word", font=("Consolas", 10))
logbox.pack(fill="both", expand=True, padx=8, pady=(0, 8))


def drain() -> None:
    try:
        while True:
            line = LOG_Q.get_nowait()
            logbox.insert("end", line + "\n")
            logbox.see("end")
    except queue.Empty:
        pass
    root.after(100, drain)


drain()
root.mainloop()
