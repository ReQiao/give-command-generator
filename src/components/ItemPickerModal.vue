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

const cardRef = ref<HTMLElement | null>(null);
const innerRef = ref<HTMLElement | null>(null);

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
  const card = cardRef.value;
  const inner = innerRef.value;
  const originEl = props.origin;
  const skip = props.animate === false || prefersReducedMotion || !card || !inner || !originEl;
  if (skip) {
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

  originEl.style.visibility = "hidden";
  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, 16));
  inner.style.opacity = "0";

  const duration = 480;
  const TRAVEL = 0.3;
  const UNFURL = 0.68;
  const OVERSHOOT = 0.8;

  const boxAnim = card.animate(
    [
      { ...boxStyle(from, 16), offset: 0 },
      { ...boxStyle(travelPoint, 16), offset: TRAVEL },
      { ...boxStyle(to, 26), offset: UNFURL },
      { ...boxStyle(scaledBox(to, 1.03), 27), offset: OVERSHOOT },
      { ...boxStyle(scaledBox(to, 0.994), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.4 },
      { ...boxStyle(scaledBox(to, 1.003), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.68 },
      { ...boxStyle(to, 26), offset: 1 },
    ],
    { duration, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "forwards" },
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

  (el as HTMLElement).animate(
    [
      { opacity: 0, offset: 0 },
      { opacity: 0, offset: TRAVEL },
      { opacity: 1, offset: UNFURL },
      { opacity: 1, offset: 1 },
    ],
    { duration, fill: "forwards" },
  );

  boxAnim.onfinish = () => {
    resetCardBox(card);
    inner.style.opacity = "";
    done();
  };
}

function onLeave(el: Element, done: () => void) {
  const card = cardRef.value;
  const inner = innerRef.value;
  const originEl = props.origin;
  const skip = props.animate === false || prefersReducedMotion || !card || !inner || !originEl;
  if (skip) {
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

  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, 26));

  const duration = 430;
  const SHRINK = 0.3;

  const boxAnim = card.animate(
    [
      { ...boxStyle(from, 26), offset: 0 },
      { ...boxStyle(shrinkPoint, 16), offset: SHRINK },
      { ...boxStyle(to, 16), offset: 1 },
    ],
    { duration, easing: "cubic-bezier(0.4,0,0.2,1)", fill: "forwards" },
  );

  inner.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0, offset: SHRINK * 0.6 },
      { opacity: 0, offset: 1 },
    ],
    { duration, easing: "ease-in", fill: "forwards" },
  );

  (el as HTMLElement).animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 0, offset: SHRINK * 0.6 },
      { opacity: 0, offset: 1 },
    ],
    { duration, fill: "forwards" },
  );

  boxAnim.onfinish = () => {
    originEl.style.visibility = "";
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
      <div v-if="open" class="modal-overlay" @click.self="open = false">
        <div ref="cardRef" class="modal-card picker-card" @keydown="onKeydown">
          <div ref="innerRef" class="picker-inner">
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
.picker-card {
  overflow: hidden;
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
