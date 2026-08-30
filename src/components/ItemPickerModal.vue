<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { CatalogRow } from "../data/catalog";
import { matches } from "../logic/builder";

const props = withDefaults(
  defineProps<{
    catalog: readonly CatalogRow[];
    current?: string;
    title?: string;
    /** 一次最多渲染多少项，超出提示用搜索或分类收窄，避免上千个节点拖慢手机 */
    renderLimit?: number;
    /** 触发弹窗的按钮元素：弹窗从它的位置"流出来"，关闭时收回它的位置 */
    origin?: HTMLElement | null;
    /** 同 AiPanel 的 :animate 约定：关了界面动画时直接跳过展开/收回动效 */
    animate?: boolean;
  }>(),
  {
    current: "",
    title: "选择物品",
    renderLimit: 300,
    origin: null,
    animate: true,
  },
);

const prefersReducedMotion =
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * 展开/收回动画要动的那几个节点，从过渡元素本身查，**不要用模板 ref**。
 *
 * 【这里踩过一次，别改回去】原来用的是 `ref="cardRef"` 那套。Vue 在关闭
 * （`v-if` 变 false）时，是**先把子树里的模板 ref 置空、再调 leave 钩子**的
 * ——钩子拿到的 `cardRef.value` 已经是 null。于是收回动画每次都悄悄走进
 * "拿不到节点就直接淡出" 的兜底分支：不仅那套精心调过的收缩动画从来没播过，
 * 兜底分支还不会把 origin 按钮的 visibility 收回来，结果关掉物品选择框之后
 * 「选择…」按钮就凭空消失了，直到下次整块重渲染才回来。
 *
 * `el` 是过渡元素本身，Vue 保证它在钩子里还在 DOM 上，从它往下查一定拿得到。
 */
function partsOf(el: Element) {
  const card = el.querySelector<HTMLElement>(".picker-card");
  const scrim = el.querySelector<HTMLElement>(".picker-scrim");
  const inner = el.querySelector<HTMLElement>(".picker-inner");
  const brand = el.querySelector<HTMLElement>(".picker-brand");
  const brandLabel = el.querySelector<HTMLElement>(".picker-brand-label");
  return { card, inner, brand, brandLabel, scrim };
}

function boxOf(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

type Box = ReturnType<typeof boxOf>;

function boxStyle(box: Box, radius: number) {
  return {
    left: `${box.x}px`,
    top: `${box.y}px`,
    width: `${box.w}px`,
    height: `${box.h}px`,
    borderRadius: `${radius}px`,
  };
}

/** 以 centerOn 为中心，放一个 w×h 大小的框——用来算"移到中点时保持原尺寸"的过渡框 */
function centeredBox(w: number, h: number, centerOn: Box): Box {
  return { x: centerOn.x + centerOn.w / 2 - w / 2, y: centerOn.y + centerOn.h / 2 - h / 2, w, h };
}

/** 以 box 为中心整体放大/缩小 factor 倍——回弹动画靠这个和主尺寸补间接在一起，不单开一段 */
function scaledBox(box: Box, factor: number): Box {
  const w = box.w * factor;
  const h = box.h * factor;
  return { x: box.x + box.w / 2 - w / 2, y: box.y + box.h / 2 - h / 2, w, h };
}

/**
 * 把弹窗打扮成"就是那颗按钮"。
 *
 * 展开的第一段里，弹窗和按钮一样大、停在按钮原地——但它长着一张玻璃脸，
 * 而真按钮是实心的品牌色。不遮住的话，那一瞬间会读成"按钮消失了，另一块
 * 玻璃出现了"，正是这套动画想避免的两个物体感。所以在弹窗内部盖一层，
 * 直接抄按钮的背景和文字，等它飞到中间、准备长大之前再淡掉。
 *
 * 抄的是 computed style 而不是写死一份颜色：按钮的皮肤以后在 CSS 里怎么改，
 * 这层跟着变，不会哪天悄悄对不上。
 */
function paintBrand(brand: HTMLElement, label: HTMLElement, origin: HTMLElement) {
  const cs = getComputedStyle(origin);
  brand.style.background = cs.backgroundImage === "none" ? cs.backgroundColor : cs.backgroundImage;
  label.style.color = cs.color;
  label.style.font = cs.font;
  label.textContent = (origin.textContent ?? "").trim();
}

/**
 * 变形期间把这块玻璃摘掉。
 *
 * backdrop-filter 的位移图是按元素当前尺寸生成的，而这套动画每一帧都在改
 * width/height——ResizeObserver 会跟着每帧回调一次，逐帧重新生成一整张
 * 位移图再写回内联样式。这是项目里已经付过学费的那类组合（见 glass.ts 里
 * clear() 上面那段），代价刚好落在最需要流畅的 480ms 上。
 *
 * 变形的头一段本来就被品牌色覆盖层挡着，后一段在长大，看不出少了折射。
 */
function glassOff(card: HTMLElement) {
  card.dataset.glassOff = "1";
}
function glassOn(card: HTMLElement) {
  delete card.dataset.glassOff;
}

/**
 * 把内容层的尺寸钉成定值。
 *
 * 【为什么必须钉】变形动画逐帧改的是卡片的 width/height，而 `.picker-inner`
 * 是 100%×100%——于是里面那三百个网格按钮每一帧都要重新布局一次。实测收回
 * 动画因此只跑出个位数的帧（430ms 里 2~4 帧），整段读起来是"卡了一下然后
 * 弹窗没了"，不是收回。
 *
 * 钉成最终尺寸之后，内容层在整段变形里一动不动，卡片外框缩到多小都只是把它
 * 裁掉——`.picker-card` 上的 overflow:hidden 就是为这件事准备的。
 */
function pinInner(inner: HTMLElement, box: Box) {
  inner.style.width = `${box.w}px`;
  inner.style.height = `${box.h}px`;
}
function unpinInner(inner: HTMLElement) {
  inner.style.width = "";
  inner.style.height = "";
}

function resetCardBox(card: HTMLElement) {
  card.style.position = "";
  card.style.margin = "";
  card.style.overflow = "";
  card.style.left = "";
  card.style.top = "";
  card.style.width = "";
  card.style.height = "";
  card.style.borderRadius = "";
}

function onEnter(el: Element, done: () => void) {
  const { card, inner, brand, brandLabel, scrim } = partsOf(el);
  const originEl = props.origin;
  const skip =
    props.animate === false ||
    prefersReducedMotion ||
    !card ||
    !inner ||
    !originEl ||
    !brand ||
    !brandLabel ||
    !scrim;
  if (skip) {
    if (scrim) scrim.style.opacity = "1";
    const anim = (el as HTMLElement).animate([{ opacity: 0 }, { opacity: 1 }], {
      duration: prefersReducedMotion ? 1 : 160,
      easing: "ease",
    });
    anim.onfinish = done;
    return;
  }

  const from = boxOf(originEl);
  const to = boxOf(card);
  const travelPoint = centeredBox(from.w, from.h, to);
  // 先量再改：一旦把卡片改成按钮大小，内容层的尺寸就跟着塌了，量不到目标值。
  pinInner(inner, boxOf(inner));

  // 真按钮要藏起来：弹窗这时正顶着它的皮和它的字站在同一个位置，
  // 底下再露出一个一模一样的按钮就穿帮了。
  originEl.style.visibility = "hidden";
  glassOff(card);
  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, 16));
  inner.style.opacity = "0";
  paintBrand(brand, brandLabel, originEl);
  brand.style.opacity = "1";

  const duration = 480;
  /** 三段的分界，单位是**实际时间**的比例（见下面为什么主曲线必须是 linear）。 */
  const TRAVEL = 0.34;
  const UNFURL = 0.74;
  const OVERSHOOT = 0.84;

  /*
   * 【主曲线必须是 linear，缓动挂在每个关键帧上】
   *
   * 关键帧的 offset 走的是**缓动之后**的进度，不是时间。原来整条动画挂一条
   * cubic-bezier(0.16,1,0.3,1)（expo-out，极度前重），实测：offset 0.3 出现在
   * 第 26ms，offset 0.68 出现在第 79ms——也就是说"先平移、再膨胀"这两段在
   * 480ms 的动画里 79ms 就演完了，剩下 400ms 全在演那点几乎看不见的回弹抖动。
   * 逐帧截图里第 60ms 的画面弹窗已经是满尺寸的，整个"从按钮里飞出来"根本没人
   * 看得到。
   *
   * 所以主时钟改成 linear，offset 就等于时间比例，分段时长一目了然：
   * 平移 163ms → 膨胀 192ms → 回弹 77ms。缓动改挂在各关键帧上（WAAPI 里
   * 关键帧的 easing 管的是"从这一帧到下一帧"这段），衔接处特意留了速度：
   * 平移段收尾用的曲线末端不归零，接上膨胀段时不会有"停一下再长大"的顿挫。
   */
  const boxAnim = card.animate(
    [
      { ...boxStyle(from, 16), offset: 0, easing: "cubic-bezier(0.4,0.05,0.35,1)" },
      { ...boxStyle(travelPoint, 16), offset: TRAVEL, easing: "cubic-bezier(0.25,0.6,0.35,1)" },
      { ...boxStyle(to, 26), offset: UNFURL, easing: "ease-out" },
      { ...boxStyle(scaledBox(to, 1.03), 27), offset: OVERSHOOT, easing: "ease-in-out" },
      { ...boxStyle(scaledBox(to, 0.994), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.4 },
      { ...boxStyle(scaledBox(to, 1.003), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.68 },
      { ...boxStyle(to, 26), offset: 1 },
    ],
    { duration, easing: "linear", fill: "forwards" },
  );

  // 品牌色这层要**赶在开始长大之前**交接完，而不是边长大边淡。
  // 用 ease-out（先快后慢）：ease-in 会让颜色在平移的大半程里都还接近满亮，
  // 快到中点才骤然褪掉，读起来像"到了才变色"。
  // 字比底色更早退场——字还在、框已经在变形，会显得字被拉扯。
  brandLabel.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0, offset: TRAVEL * 0.35 },
      { opacity: 0, offset: 1 },
    ],
    { duration, easing: "ease-out", fill: "forwards" },
  );
  brand.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0.05, offset: TRAVEL * 0.55 },
      { opacity: 0, offset: 1 },
    ],
    { duration, easing: "ease-out", fill: "forwards" },
  );

  inner.animate(
    [
      { opacity: 0, offset: 0 },
      { opacity: 0, offset: TRAVEL },
      { opacity: 1, offset: UNFURL },
      { opacity: 1, offset: 1 },
    ],
    { duration, easing: "ease-out", fill: "forwards" },
  );

  // 变暗的是**遮罩**，不是整个过渡元素。
  //
  // 【这里踩过一次】原来动的是 `el`（也就是 .modal-overlay）的 opacity——可它
  // 同时是弹窗的父节点，opacity 0 会把弹窗一起按住。于是"从按钮里飞出来"的
  // 第一段整段都是隐形的，画面上只是一块玻璃凭空在中间长出来，这套动画最想
  // 表达的那件事恰好被自己遮掉了。现在遮罩是单独一层，弹窗从第 0 帧就看得见。
  const scrimAnim = scrim.animate(
    [
      { opacity: 0, offset: 0 },
      { opacity: 0, offset: TRAVEL },
      { opacity: 1, offset: UNFURL },
      { opacity: 1, offset: 1 },
    ],
    { duration, fill: "forwards" },
  );

  boxAnim.onfinish = () => {
    /*
     * 【必须 cancel，光清内联样式不够】fill:"forwards" 会让动画定格在最后一帧
     * 继续生效，而且它在层叠里压过内联样式。这里踩过一次：`.picker-card` 为了
     * 承载品牌色覆盖层加了 position:relative，于是动画定格的那份 `left:120px;
     * top:60px` 就变成了相对偏移——弹窗稳稳地朝右下角挪了一整段，右边和底边
     * 都出了屏。cancel 掉才算真正把这几个属性还给 CSS。
     */
    boxAnim.cancel();
    scrimAnim.cancel();
    resetCardBox(card);
    unpinInner(inner);
    inner.style.opacity = "";
    brand.style.opacity = "0";
    // 遮罩的 opacity 本来靠动画的定格撑着，撤了就得自己写回来。
    scrim.style.opacity = "1";
    // 尺寸定了才把折射装回来：这时只需要生成一张位移图，不是每帧一张。
    glassOn(card);
    done();
  };
}

function onLeave(el: Element, done: () => void) {
  const { card, inner, brand, brandLabel, scrim } = partsOf(el);
  const originEl = props.origin;
  const skip =
    props.animate === false ||
    prefersReducedMotion ||
    !card ||
    !inner ||
    !originEl ||
    !brand ||
    !brandLabel ||
    !scrim;
  if (skip) {
    // 兜底也要把按钮放出来：展开时动画还开着（按钮被藏起来了），关的时候用户
    // 刚好把「启用界面动画」关掉——这条路径不还原的话，按钮就永久消失了。
    if (originEl) originEl.style.visibility = "";
    const anim = (el as HTMLElement).animate([{ opacity: 1 }, { opacity: 0 }], {
      duration: prefersReducedMotion ? 1 : 140,
      easing: "ease",
    });
    anim.onfinish = done;
    return;
  }

  const from = boxOf(card);
  const to = boxOf(originEl);
  const shrinkPoint = centeredBox(to.w, to.h, from);
  pinInner(inner, boxOf(inner));

  glassOff(card);
  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, 26));
  paintBrand(brand, brandLabel, originEl);
  brand.style.opacity = "0";

  const duration = 430;
  /** 同展开：主时钟是 linear，所以这就是实际时间比例——收缩 146ms，平移 284ms。 */
  const SHRINK = 0.34;

  const boxAnim = card.animate(
    [
      { ...boxStyle(from, 26), offset: 0, easing: "cubic-bezier(0.4,0,0.5,1)" },
      { ...boxStyle(shrinkPoint, 16), offset: SHRINK, easing: "cubic-bezier(0.3,0,0.2,1)" },
      { ...boxStyle(to, 16), offset: 1 },
    ],
    { duration, easing: "linear", fill: "forwards" },
  );

  // 收回时品牌色是在**收缩这一段里**逐渐浮上来的，不是缩完了才啪一下换成
  // 按钮的样子——那样最后一帧会跳色。字压后半拍再出来，理由同展开时相反：
  // 框还在变形的时候就摆上字，字会被压扁。
  brand.animate(
    [
      { opacity: 0, offset: 0 },
      { opacity: 0.95, offset: SHRINK },
      { opacity: 1, offset: 1 },
    ],
    { duration, easing: "ease-in", fill: "forwards" },
  );
  brandLabel.animate(
    [
      { opacity: 0, offset: 0 },
      { opacity: 0, offset: SHRINK * 0.5 },
      { opacity: 1, offset: SHRINK },
      { opacity: 1, offset: 1 },
    ],
    { duration, easing: "ease-out", fill: "forwards" },
  );

  inner.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0, offset: SHRINK * 0.6 },
      { opacity: 0, offset: 1 },
    ],
    { duration, easing: "ease-in", fill: "forwards" },
  );

  scrim.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0, offset: SHRINK * 0.6 },
      { opacity: 0, offset: 1 },
    ],
    { duration, fill: "forwards" },
  );

  boxAnim.onfinish = () => {
    // 弹窗已经缩回按钮的位置和大小、并且换上了按钮的皮，这时才把真按钮放出来——
    // 早一帧放就会同时看见两个按钮。
    originEl.style.visibility = "";
    unpinInner(inner);
    brand.style.opacity = "0";
    glassOn(card);
    done();
  };
}

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ select: [name: string] }>();

const ALL = "全部";
const query = ref("");
const category = ref(ALL);
const searchInput = ref<HTMLInputElement | null>(null);

function categoryOf(row: CatalogRow): string {
  return typeof row[3] === "string" ? row[3] : "其他";
}

const categories = computed(() => {
  const counts = new Map<string, number>();
  for (const row of props.catalog) {
    const cat = categoryOf(row);
    counts.set(cat, (counts.get(cat) ?? 0) + 1);
  }
  return [
    { name: ALL, count: props.catalog.length },
    ...[...counts].sort((a, b) => b[1] - a[1]).map(([name, count]) => ({ name, count })),
  ];
});

const filtered = computed(() => {
  const cat = category.value;
  const q = query.value.trim();
  return props.catalog.filter(
    (row) => (cat === ALL || categoryOf(row) === cat) && (!q || matches(row, q)),
  );
});

const shown = computed(() => filtered.value.slice(0, props.renderLimit));
const hidden = computed(() => filtered.value.length - shown.value.length);

watch(open, async (isOpen) => {
  if (!isOpen) return;
  query.value = "";
  category.value = ALL;
  await nextTick();
  searchInput.value?.focus();
});

function choose(row: CatalogRow) {
  emit("select", String(row[1]));
  open.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    open.value = false;
  } else if (event.key === "Enter" && shown.value.length) {
    event.preventDefault();
    choose(shown.value[0]);
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition :css="false" @enter="onEnter" @leave="onLeave">
      <div v-if="open" class="modal-overlay picker-overlay" @click.self="open = false">
        <div class="picker-scrim"></div>
        <div class="modal-card picker-card" @keydown="onKeydown">
          <div class="picker-brand" aria-hidden="true">
            <span class="picker-brand-label"></span>
          </div>
          <div class="picker-inner">
            <div class="picker-head">
              <h2>{{ props.title }}</h2>
              <button class="picker-close" type="button" aria-label="关闭" @click="open = false">×</button>
            </div>

            <input
              ref="searchInput"
              v-model="query"
              class="picker-search"
              placeholder="搜索中文名、英文 ID 或分类"
            />

            <div class="picker-cats">
              <button
                v-for="cat in categories"
                :key="cat.name"
                :class="{ active: category === cat.name }"
                type="button"
                @click="category = cat.name"
              >
                {{ cat.name }}<small>{{ cat.count }}</small>
              </button>
            </div>

            <div class="picker-grid">
              <button
                v-for="row in shown"
                :key="String(row[0])"
                :class="{ selected: props.current === row[1] }"
                type="button"
                @click="choose(row)"
              >
                <span class="picker-name">{{ row[1] }}</span>
                <small>{{ String(row[0]).replace("minecraft:", "") }}</small>
              </button>

              <p v-if="!shown.length" class="picker-empty">没有匹配的结果</p>
            </div>

            <p class="picker-foot">
              <span v-if="hidden > 0">显示前 {{ shown.length }} 项，还有 {{ hidden }} 项——继续输入或选择分类以收窄范围</span>
              <span v-else>共 {{ filtered.length }} 项</span>
            </p>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* 弹窗展开/收回时框体本身由 onEnter/onLeave 里的 element.animate() 直接接管
   left/top/width/height/border-radius，这里只需要让内容能在框体变形的过程中
   独立淡入淡出（picker-inner），并且外层不会在收缩阶段把网格内容露出来。 */
/* 遮罩从 .modal-overlay 上剥出来，单独一层。
   过渡元素本身不能再带底色和模糊——它是弹窗的父节点，动它的 opacity 会把
   弹窗一起按住（见 onEnter 里那段注释）。 */
.picker-overlay {
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.picker-scrim {
  position: absolute;
  background: rgba(3, 8, 22, 0.42);
  opacity: 0;
  backdrop-filter: blur(10px) saturate(120%);
  -webkit-backdrop-filter: blur(10px) saturate(120%);
  inset: 0;
  /* 点空白处关闭靠父元素的 @click.self，遮罩不能把点击吃掉 */
  pointer-events: none;
}

.picker-card {
  /* 品牌色覆盖层按这块卡片定位；变形时这里会被改成 fixed，同样是包含块。 */
  position: relative;
  overflow: hidden;
}

/* 品牌色覆盖层：铺满整块弹窗，展开的第一段里它就是"那颗按钮"的脸。
   inset:0 + border-radius:inherit 让它天然跟着弹窗一起变形，不用另外追位置。
   平时 opacity:0 且 pointer-events:none，不参与任何交互。 */
.picker-brand {
  position: absolute;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: inherit;
  opacity: 0;
  inset: 0;
  pointer-events: none;
}

.picker-brand-label {
  white-space: nowrap;
}

.picker-inner {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
  height: 100%;
  min-height: 0;
}
</style>
