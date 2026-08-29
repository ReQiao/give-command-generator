/**
 * 液态玻璃：边缘位移折射。
 *
 * # 这套和之前那套的区别
 *
 * 之前用的是 `feTurbulence` 程序化噪声做位移——整块面板都在轻微扭动，观感是
 * "老式压花玻璃窗"。这套改成**按元素尺寸算出来的边缘位移图**：中间是中性灰
 * （不位移），只有靠近边框那一圈有梯度，于是光只在边缘被弯折，中间保持笔直。
 * 那才是"厚玻璃的倒角"，也就是通常说的液态玻璃。
 *
 * 实测两者的**性能几乎一样**（headless Chromium，340×560 的面板上盖一条动的
 * 扫光：feTurbulence 16fps / 最长卡顿 119ms，位移图 17fps / 91ms，带色散
 * 16fps / 102ms，静止基线 63fps）。也就是说贵的不是"算噪声"，而是
 * `backdrop-filter` 本身每帧重新采样并重滤背景。
 *
 * **所以换成这套不会解决卡顿**：`.ai-card.ignite-flat` 那条"动画期间摘掉玻璃"
 * 的处理必须继续留着，见 style.css 里那段注释。
 *
 * # 为什么是全局安装而不是 Vue 指令
 *
 * 位移图必须按元素的**实际像素尺寸**生成，尺寸对不上边缘倒角就会错位。这意味着
 * 每个面板都要 ResizeObserver。做成指令的话要去改 App.vue、AuthModal、
 * ItemPickerModal、RichTextEditor（里面有五个弹窗）、CustomSelect、CatalogCombo、
 * InfoTip……八九个文件，而且以后新加一个弹窗还得记得加指令。
 *
 * 全局装一次、按选择器认领，新弹窗自动就有玻璃，不会漏。
 *
 * # 圆角和厚度从 CSS 里读
 *
 * `border-radius` 直接读计算样式——这样位移图的圆角**永远**和元素真实的圆角一致，
 * 不会出现"CSS 改了圆角、玻璃倒角还停在旧值"这种只能靠肉眼发现的漂移。
 * 厚度/强度/色散读 CSS 自定义属性，于是每类面板的调参仍然留在 style.css 里，
 * 不用回来改这个文件。
 */

// ---------------------------------------------------------------- 位移图

export interface LensOptions {
  width: number;
  height: number;
  radius: number;
  /** 倒角厚度（px）。越大，"玻璃越厚"，边缘那圈弯折的范围越宽。 */
  depth: number;
  /** 位移强度。决定边缘把背景拉弯多少。 */
  strength: number;
  /** 色散：RGB 三通道用不同强度各位移一次，边缘出现分色。0 = 关闭。 */
  chromaticAberration: number;
}

/**
 * 生成位移图。
 *
 * 原理：R 通道编码 x 位移、G 通道编码 y 位移，128 为中性（不位移）。
 * 两条线性渐变分别铺满 R 和 G，`mix-blend-mode: screen` 把它们叠起来；
 * 然后在中间盖一块 `#808080`（=128，中性）的圆角矩形并模糊，
 * 把内部"擦"回不位移，只在边缘留下 `depth` 宽的一圈梯度。
 */
function displacementMap({ width, height, radius, depth }: Omit<LensOptions, "strength" | "chromaticAberration">): string {
  const svg = `<svg height="${height}" width="${width}" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">
  <style>.mix { mix-blend-mode: screen; }</style>
  <defs>
    <linearGradient id="Y" x1="0" x2="0" y1="${Math.ceil((radius / height) * 15)}%" y2="${Math.floor(100 - (radius / height) * 15)}%">
      <stop offset="0%" stop-color="#0F0"/><stop offset="100%" stop-color="#000"/>
    </linearGradient>
    <linearGradient id="X" x1="${Math.ceil((radius / width) * 15)}%" x2="${Math.floor(100 - (radius / width) * 15)}%" y1="0" y2="0">
      <stop offset="0%" stop-color="#F00"/><stop offset="100%" stop-color="#000"/>
    </linearGradient>
  </defs>
  <rect x="0" y="0" height="${height}" width="${width}" fill="#808080"/>
  <g filter="blur(2px)">
    <rect x="0" y="0" height="${height}" width="${width}" fill="#000080"/>
    <rect x="0" y="0" height="${height}" width="${width}" fill="url(#Y)" class="mix"/>
    <rect x="0" y="0" height="${height}" width="${width}" fill="url(#X)" class="mix"/>
    <rect x="${depth}" y="${depth}" height="${height - 2 * depth}" width="${width - 2 * depth}"
          fill="#808080" rx="${radius}" ry="${radius}" filter="blur(${depth}px)"/>
  </g>
</svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

/**
 * 生成折射滤镜，返回可直接塞进 `backdrop-filter: url('…')` 的串。
 *
 * 色散关闭时**只做一遍** `feDisplacementMap`。开启时要做三遍（RGB 各一遍）
 * 外加三个 `feColorMatrix` 和两个 `feBlend`——多出来的这些在
 * `chromaticAberration === 0` 时是纯浪费，所以这里分了两条路。
 */
function buildFilter(o: LensOptions): string {
  const map = displacementMap(o);
  const { width, height, strength, chromaticAberration: ca } = o;

  const body = ca > 0
    ? `<feDisplacementMap in="SourceGraphic" in2="m" scale="${strength + ca * 2}" xChannelSelector="R" yChannelSelector="G"/>
       <feColorMatrix type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="r"/>
       <feDisplacementMap in="SourceGraphic" in2="m" scale="${strength + ca}" xChannelSelector="R" yChannelSelector="G"/>
       <feColorMatrix type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="g"/>
       <feDisplacementMap in="SourceGraphic" in2="m" scale="${strength}" xChannelSelector="R" yChannelSelector="G"/>
       <feColorMatrix type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="b"/>
       <feBlend in="r" in2="g" mode="screen"/>
       <feBlend in2="b" mode="screen"/>`
    : `<feDisplacementMap in="SourceGraphic" in2="m" scale="${strength}" xChannelSelector="R" yChannelSelector="G"/>`;

  const svg = `<svg height="${height}" width="${width}" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">
  <defs><filter id="d" color-interpolation-filters="sRGB">
    <feImage x="0" y="0" height="${height}" width="${width}" href="${map}" result="m"/>
    ${body}
  </filter></defs>
</svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg) + "#d";
}

/**
 * 带缓存的滤镜生成。
 *
 * 一份滤镜串是 4~6 KB（里面还嵌着位移图的 data URI），而且**尺寸变一像素就是
 * 另一个 URL**，浏览器要重新解码、重新栅格化。窗口拖动缩放时如果每一帧都生成
 * 新串，那就是每帧一次全量重建。
 *
 * 缓存按完整参数做键。不做尺寸量化——量化会让位移图和元素边缘对不齐，
 * 倒角明显偏移，那是肉眼可见的错误；缓存未命中只是慢一点。真正的抖动由
 * 调用方的 rAF 合并来压（见 refresh）。
 */
const cache = new Map<string, string>();
const CACHE_LIMIT = 64;

export function lensFilter(o: LensOptions): string {
  const key = `${o.width}x${o.height}r${o.radius}d${o.depth}s${o.strength}c${o.chromaticAberration}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const built = buildFilter(o);
  // 简单的 FIFO 上限：窗口连续缩放会产生大量一次性尺寸，不设上限会一直涨。
  if (cache.size >= CACHE_LIMIT) cache.delete(cache.keys().next().value as string);
  cache.set(key, built);
  return built;
}

/** 只给测试用：清掉缓存，好断言"同参数只构建一次"。 */
export function _clearLensCache() {
  cache.clear();
}

// ---------------------------------------------------------------- 能力探测

/**
 * 这个引擎支持 `backdrop-filter` 里引用 SVG 滤镜吗。
 *
 * 只有 Chromium 支持。WebKit（macOS 上 Tauri 用的 WKWebView）和 Gecko 都不支持，
 * 而且**不是报错、是整条 backdrop-filter 声明失效**——直接看穿，比不加还难看。
 * 我们的发布流水线有 macOS Intel 和 arm64 两个 job，所以这条降级不是可选项。
 *
 * 判据用 `navigator.userAgentData` 的存在性，不用 UA 字符串正则：
 * 这个 API 只有 Chromium 实现，是个结构化的事实，不会因为某个浏览器在 UA 里
 * 塞了 "Safari" 或 "Chrome" 字样就误判——而 UA 正则恰恰天天栽在这上面
 * （Edge 的 UA 里同时有 Chrome 和 Safari）。
 *
 * `CSS.supports` 在这里没用：Safari 能**解析** `url()` 语法、返回 true，
 * 只是运行时不生效，探测不出来。
 */
/**
 * 手动覆盖开关，`localStorage` 里的 `soul-lantern-glass`：
 *   "off" —— 强制走降级（纯 blur，没有折射）
 *   "on"  —— 强制上透镜
 *   其它/没设 —— 自动探测
 *
 * 两个用途，都不是可有可无的：
 *
 * 1. **让降级路径可验证**。真正需要降级的是 macOS（WKWebView），但手上不一定有
 *    Mac。降级的样子和平台无关，在 Windows 上把这个设成 "off" 就能看到 macOS
 *    用户看到的完全一样的界面——否则那条路径只能靠"发出去等人反馈"来测。
 * 2. **逃生开关**。万一某台机器上透镜出问题（显卡驱动、某个 WebView2 版本），
 *    用户不用等新版本，开发者电话里指导他在控制台敲一行就能先用起来。
 */
function override(): "on" | "off" | null {
  try {
    const v = localStorage.getItem("soul-lantern-glass");
    return v === "on" || v === "off" ? v : null;
  } catch {
    // 隐私模式/禁用了存储时 localStorage 会抛，不能让它拖垮整个玻璃
    return null;
  }
}

export const supportsSvgBackdropFilter: boolean = (() => {
  if (typeof navigator === "undefined") return false;
  const forced = override();
  if (forced) return forced === "on";
  if ("userAgentData" in navigator) return true;
  // 兜底：老一点的 Chromium 没有 userAgentData
  const ua = navigator.userAgent;
  return /Chrom(e|ium)|Edg\//.test(ua) && !/(^|[^n])Gecko\/|FxiOS/.test(ua);
})();

// ---------------------------------------------------------------- 安装

/** 哪些东西算"窗口"。新加了浮层就往这里加一条，不用改组件。 */
const GLASS_SELECTOR = [
  ".card",
  ".modal-card",
  ".eula-box",
  ".combo-menu",
  ".info-bubble",
  ".toast",
  ".ai-topup-panel",
].join(",");

/** 逐类面板的调参留在 CSS 里，这里只读。 */
function num(style: CSSStyleDeclaration, prop: string, fallback: number): number {
  const v = parseFloat(style.getPropertyValue(prop));
  return Number.isFinite(v) ? v : fallback;
}

/**
 * 关掉某个元素的透镜。
 *
 * 【为什么必须有这个开关，别删】玻璃和"玻璃上面有东西在动"是天然冲突的：
 * backdrop-filter 的结果依赖背后的合成内容，上面盖着逐帧变化的东西时，
 * 浏览器每帧都要把整套滤镜重算一遍。所以点灯特效期间要把玻璃整层摘掉
 * （见 style.css 里 .ai-card.ignite-flat 那段实测数据）。
 *
 * 而这里写的是**内联样式**，优先级高过任何 CSS 规则——光在 CSS 里写
 * `.ignite-flat { backdrop-filter: none }` 是压不住它的。这一点踩过一次：
 * 换成透镜之后动画直接从 60 帧掉回 16 帧，就是因为那条 CSS 被内联样式盖住了。
 * 所以关的时候必须**清掉内联样式**，把场子还给 CSS。
 */
function clear(el: HTMLElement) {
  el.style.removeProperty("backdrop-filter");
}

/**
 * 按下时玻璃"变厚"的倍率。
 *
 * 这是从你给的那个 GlassElement 里学来的：它在 mousedown 时把 depth 除以 0.7
 * （≈ 放大 1.43 倍），松手复位。效果是按下去那一瞬间边缘的折射突然变强，
 * 像真的把一块玻璃往下按了一下。光靠 CSS 的 scale 做不出这个——scale 缩的是
 * 整个元素，而这里变的是"玻璃有多厚"。
 */
const PRESS_DEPTH = 1.45;

function apply(el: HTMLElement, pressed = el.classList.contains("glass-press")) {
  // 显式关掉的（比如点灯动画期间的 .ai-card）不碰
  if (el.dataset.glassOff === "1") {
    clear(el);
    return;
  }

  const rect = el.getBoundingClientRect();
  const w = Math.round(rect.width);
  const h = Math.round(rect.height);
  // 还没布局出来 / 被隐藏了：什么都不做，等下一次 ResizeObserver 回调
  if (w < 8 || h < 8) return;

  const cs = getComputedStyle(el);
  const depth = num(cs, "--glass-depth", 10) * (pressed ? PRESS_DEPTH : 1);
  // 倒角厚度的两倍不能超过短边，否则位移图里那个"擦回中性"的内矩形宽高会变负数
  const safeDepth = Math.max(1, Math.min(depth, Math.floor(Math.min(w, h) / 2) - 1));
  const radius = Math.min(
    num(cs, "--glass-radius", parseFloat(cs.borderTopLeftRadius) || 16),
    Math.floor(Math.min(w, h) / 2),
  );

  const filter = lensFilter({
    width: w,
    height: h,
    radius,
    depth: safeDepth,
    strength: num(cs, "--glass-strength", 44),
    chromaticAberration: num(cs, "--glass-aberration", 0),
  });

  const blur = num(cs, "--glass-blur", 12);
  const saturate = num(cs, "--glass-saturate", 180);
  const brightness = num(cs, "--glass-brightness", 1.04);
  el.style.backdropFilter =
    `url('${filter}') blur(${blur}px) saturate(${saturate}%) brightness(${brightness})`;
}

/**
 * 装上液态玻璃。整个应用调用一次。
 *
 * 不支持 SVG 滤镜的引擎（WebKit/Gecko）直接返回——那些平台上 style.css 里的
 * `-webkit-backdrop-filter` 纯模糊降级会接管，不做任何 JS 侧的事。
 */
export function installLiquidGlass(): () => void {
  if (typeof document === "undefined" || !supportsSvgBackdropFilter) return () => {};

  document.documentElement.classList.add("has-lens-glass");

  const pending = new Set<HTMLElement>();
  let raf = 0;
  /** 把同一帧里的多次尺寸变化合并成一次重建，别在拖动窗口时每个回调都生成一份滤镜。 */
  const schedule = (el: HTMLElement) => {
    pending.add(el);
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      const batch = [...pending];
      pending.clear();
      // 注意不能写成 batch.forEach(apply)：forEach 会把下标当第二个参数传进去，
      // 于是每个元素都被当成"按下状态"重算一遍。
      for (const el of batch) apply(el);
    });
  };

  const ro = new ResizeObserver((entries) => {
    for (const e of entries) schedule(e.target as HTMLElement);
  });

  /**
   * 按下 / 松手的回弹。
   *
   * 挂在 document 上做事件委托，而不是给每个面板各挂一对监听：面板是动态出现的
   * （弹窗 v-if），委托就不用管认领时机，也不用在卸载时逐个摘。
   *
   * 用 pointer 事件而不是 mouse 事件：触摸屏和触控笔一样能触发。
   * pointerup 挂在 window 上是必须的——按下之后把鼠标拖出面板再松手，
   * 光靠面板自己的 pointerup 收不到事件，那块玻璃就会一直卡在"按下"状态。
   */
  const pressed = new Set<HTMLElement>();
  const release = () => {
    for (const el of pressed) {
      el.classList.remove("glass-press");
      schedule(el);
    }
    pressed.clear();
  };
  const onDown = (e: PointerEvent) => {
    const t = e.target as HTMLElement | null;
    const el = t?.closest?.(GLASS_SELECTOR) as HTMLElement | null;
    if (!el || el.dataset.glassOff === "1") return;
    // 嵌套的情况（弹窗里的下拉菜单）只认最内层那一个，否则会一次按下去两层。
    el.classList.add("glass-press");
    pressed.add(el);
    schedule(el);
  };
  document.addEventListener("pointerdown", onDown, true);
  window.addEventListener("pointerup", release, true);
  window.addEventListener("pointercancel", release, true);
  // 焦点被抢走（比如按下之后切窗口）也要复位，不然回来看到一块按扁的玻璃
  window.addEventListener("blur", release);

  const seen = new WeakSet<HTMLElement>();
  const claim = (root: ParentNode) => {
    const list: HTMLElement[] = [];
    if (root instanceof HTMLElement && root.matches(GLASS_SELECTOR)) list.push(root);
    root.querySelectorAll<HTMLElement>(GLASS_SELECTOR).forEach((el) => list.push(el));
    for (const el of list) {
      if (seen.has(el)) continue;
      seen.add(el);
      ro.observe(el);
      schedule(el);
    }
  };

  claim(document);

  // 两件事都靠这个观察者：
  //   childList —— 弹窗是按需挂载的（v-if），后来出现的节点也要认领
  //   attributes —— data-glass-off 被切换时，立刻摘掉/装回内联滤镜
  const mo = new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === "attributes") {
        const el = r.target as HTMLElement;
        if (el.dataset.glassOff === "1") clear(el);
        else schedule(el);
        continue;
      }
      r.addedNodes.forEach((n) => {
        if (n instanceof HTMLElement) claim(n);
      });
    }
  });
  mo.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["data-glass-off"],
  });

  return () => {
    mo.disconnect();
    ro.disconnect();
    document.removeEventListener("pointerdown", onDown, true);
    window.removeEventListener("pointerup", release, true);
    window.removeEventListener("pointercancel", release, true);
    window.removeEventListener("blur", release);
    if (raf) cancelAnimationFrame(raf);
    document.documentElement.classList.remove("has-lens-glass");
  };
}
