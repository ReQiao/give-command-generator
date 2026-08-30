/**
 * 开场动画：先只有背景，界面再一块块落位。
 *
 * # 节奏
 *
 * 1. 一开始整个 `.app-shell` 是透明的，屏幕上只有那盏灯和它的光晕。
 * 2. 稍等一拍（让人看清背景），外层玻璃淡入。
 * 3. 每个分区块**从自己所在的那一侧更外面**淡入——右半边的块从更右边进来，
 *    左半边的从更左边。观感是"它们本来被甩到屏幕边缘，现在归位"。
 * 4. 归位时先冲过头一点，再退回来。
 * 5. 落位后播一次"按一下这块面板"的回弹（复用 `.glass-press`，不另写一套）。
 *
 * # 为什么用 Web Animations API 而不是 CSS
 *
 * 每个块的位移方向和距离都取决于它在屏幕上的实际位置——CSS 写不出来，要么给每个
 * 块手写一条规则（布局一变就错），要么在 JS 里拼 keyframes 字符串塞进 style 标签
 * （更糟）。WAAPI 直接接受运行时算出来的数值，而且 `finished` 是个 Promise，
 * 串"落位 → 回弹"这两步不用猜时间。
 *
 * # 只动 transform 和 opacity
 *
 * 这两个是合成器属性，不触发重排重绘。开场这一下同时有四五个块在动，
 * 换成动 left/top 会直接卡给用户看——项目里已经为这件事付过一次学费
 * （见 style.css 里 .ai-sweep 那段）。
 */

/** 块从多远的地方进来。按它偏离屏幕中心的程度算，再夹到这个上限。 */
const MAX_OFFSET_PX = 90;
/** 冲过头多少（相对进场位移的比例）。太大就成了甩，太小看不出来。 */
const OVERSHOOT = 0.22;

const SHELL_FADE_MS = 420;
/** 背景独处的时间。太短看不清那盏灯，太长像卡住了。 */
const BACKDROP_HOLD_MS = 260;
const BLOCK_MS = 560;
const STAGGER_MS = 70;

function motionOff(): boolean {
  if (document.documentElement.classList.contains("no-motion")) return true;
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * 这个块该从哪个方向进来。
 *
 * 取它的中心相对视口中心的偏移，归一化之后乘上限。所以越靠边的块进场位移越大，
 * 正中间的块几乎只是淡入——这正是"向屏幕边缘放错了"的那种感觉，
 * 而不是所有块整齐地朝同一个方向平移。
 */
function offsetFor(el: HTMLElement): { dx: number; dy: number } {
  const r = el.getBoundingClientRect();
  const cx = (r.left + r.right) / 2 - window.innerWidth / 2;
  const cy = (r.top + r.bottom) / 2 - window.innerHeight / 2;
  const norm = (v: number, half: number) => Math.max(-1, Math.min(1, v / Math.max(1, half)));
  return {
    dx: norm(cx, window.innerWidth / 2) * MAX_OFFSET_PX,
    dy: norm(cy, window.innerHeight / 2) * (MAX_OFFSET_PX * 0.45),
  };
}

/** 落位后按一下自己。复用 `.glass-press`，不另写一套放大缩小。 */
function pressOnce(el: HTMLElement) {
  el.classList.add("glass-press");
  window.setTimeout(() => el.classList.remove("glass-press"), 120);
}

/**
 * 放一次开场动画。返回一个 Promise，全部落位后 resolve。
 *
 * 关了界面动画（或系统要求减弱动效）时直接把界面显示出来，一帧都不演——
 * 用户要的是"全都别动"，开场也算在内。
 */
export async function playIntro(shell: HTMLElement): Promise<void> {
  const blocks = [...shell.querySelectorAll<HTMLElement>(".card")];

  if (motionOff()) {
    blocks.forEach((b) => {
      b.style.opacity = "";
      b.style.transform = "";
    });
    return;
  }

  // 先量好每个块的目标位置再动手：一旦开始设 transform，getBoundingClientRect
  // 拿到的就是动画中的位置，方向会算错。
  const plan = blocks.map((el) => ({ el, ...offsetFor(el) }));

  // 起手式：块先藏起来并挪到外侧，外层玻璃也是透明的。
  // 这一步必须同步做完，否则会闪一帧完整界面。
  for (const { el, dx, dy } of plan) {
    el.style.opacity = "0";
    el.style.transform = `translate3d(${dx}px, ${dy}px, 0)`;
  }

  await new Promise((r) => setTimeout(r, BACKDROP_HOLD_MS));

  // shell 的 opacity 由 App.vue 的 introPending 绑定着（模板里就是 0，
  // 这样第一帧就是隐藏的）。这里不去动内联样式——WAAPI 动画在层叠里本来就压过
  // 内联样式，等这一整套跑完 introPending 翻 false，Vue 自己会把它摘掉。
  shell.animate([{ opacity: 0 }, { opacity: 1 }], {
    duration: SHELL_FADE_MS,
    easing: "cubic-bezier(0.22, 0.61, 0.36, 1)",
    fill: "both",
  });

  await Promise.all(
    plan.map(async ({ el, dx, dy }, i) => {
      const anim = el.animate(
        [
          { opacity: 0, transform: `translate3d(${dx}px, ${dy}px, 0)`, offset: 0 },
          // 冲过头：越过目标位置，跑到反方向一点点
          {
            opacity: 1,
            transform: `translate3d(${-dx * OVERSHOOT}px, ${-dy * OVERSHOOT}px, 0)`,
            offset: 0.62,
            easing: "cubic-bezier(0.16, 0.84, 0.3, 1)",
          },
          { opacity: 1, transform: "translate3d(0, 0, 0)", offset: 1 },
        ],
        {
          duration: BLOCK_MS,
          delay: i * STAGGER_MS,
          easing: "cubic-bezier(0.33, 1, 0.68, 1)",
          fill: "both",
        },
      );
      await anim.finished;
      // 把 fill:both 留下的定格状态撤掉，交回给 CSS——否则之后的
      // .glass-press 会被这条动画的 transform 压住，按下去没反应。
      anim.cancel();
      el.style.opacity = "";
      el.style.transform = "";
      pressOnce(el);
    }),
  );
}
