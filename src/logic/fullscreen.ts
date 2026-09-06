import { isTauri } from "@tauri-apps/api/core";

/**
 * F11 全屏。
 *
 * 桌面端（Tauri webview）里 F11 默认没有任何反应——浏览器那套全屏快捷键是
 * 浏览器外壳提供的，webview 里没有外壳，所以得自己接。走 Tauri 的窗口 API
 * 切真正的窗口全屏（需要 capabilities 里的 core:window:allow-set-fullscreen）。
 *
 * 跑在普通浏览器里时不要拦 F11：那是浏览器自己的全屏键，拦了反而把用户
 * 熟悉的行为改掉了。这里只在非 Tauri 环境下退化到标准 Fullscreen API，
 * 供页面内的「全屏」按钮之类的入口调用。
 */

async function toggleTauriFullscreen(): Promise<void> {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}

async function toggleBrowserFullscreen(): Promise<void> {
  if (document.fullscreenElement) await document.exitFullscreen();
  else await document.documentElement.requestFullscreen();
}

export async function toggleFullscreen(): Promise<void> {
  try {
    if (isTauri()) await toggleTauriFullscreen();
    else await toggleBrowserFullscreen();
  } catch {
    // 全屏被系统/浏览器拒绝（比如没有用户手势）时静默失败——这是锦上添花的
    // 功能，不该为此弹错误打断用户。
  }
}

/** 挂上 F11 监听，返回取消函数。 */
export function installFullscreenShortcut(): () => void {
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "F11" || event.ctrlKey || event.altKey || event.metaKey) return;
    // 浏览器里交给浏览器自己处理，不抢
    if (!isTauri()) return;
    event.preventDefault();
    void toggleFullscreen();
  }

  window.addEventListener("keydown", onKeydown);
  return () => window.removeEventListener("keydown", onKeydown);
}
