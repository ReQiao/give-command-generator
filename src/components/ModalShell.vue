<script setup lang="ts">
/**
 * 弹窗外壳：所有弹出层共用的「从触发元素流出来」动效 + 液态玻璃卡片。
 *
 * 动效分三段（展开）：
 *   1. TRAVEL  —— 保持触发按钮的尺寸，从按钮位置移到卡片中心
 *   2. UNFURL  —— 就地展开到卡片全尺寸，圆角同步从按钮圆角过渡到卡片圆角
 *   3. 回弹    —— 1.03 → 0.994 → 1.003 → 1.0，接在同一条补间里，不单开一段
 * 收回时原路返回（先缩到按钮尺寸，再移回按钮位置），符合「从哪来回哪去」的空间一致性。
 *
 * 卡片内容（inner）独立淡入淡出：框体变形期间内容是透明的，避免文字被拉伸。
 */
import { onBeforeUnmount, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 触发弹窗的元素：弹窗从它的位置流出来，关闭时收回它的位置。为空则退化成淡入淡出。 */
    origin?: HTMLElement | null;
    /** 关了「启用界面动画」时跳过展开/收回动效。 */
    animate?: boolean;
    /** 附加在 .modal-card 上的类，如 color-card / picker-card。 */
    cardClass?: string;
    /** 附加在内容层上的类——内容层默认是普通块级，需要撑满/弹性布局时用它。 */
    innerClass?: string;
    /** 点击遮罩关闭。RichTextEditor 里的弹窗用 mousedown 触发以避免拖选文字误关。 */
    closeOn?: "click" | "mousedown" | "none";
    /** 触发元素的圆角（展开起点）。 */
    fromRadius?: number;
    /** 卡片圆角（展开终点），需与 CSS 里 .modal-card 的圆角一致。 */
    toRadius?: number;
  }>(),
  {
    origin: null,
    animate: true,
    cardClass: "",
    innerClass: "",
    closeOn: "click",
    fromRadius: 12,
    toRadius: 24,
  },
);

const open = defineModel<boolean>("open", { required: true });

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

/** 以 centerOn 为中心放一个 w×h 的框——用来算「移到中点时保持原尺寸」的过渡框。 */
function centeredBox(w: number, h: number, centerOn: Box): Box {
  return { x: centerOn.x + centerOn.w / 2 - w / 2, y: centerOn.y + centerOn.h / 2 - h / 2, w, h };
}

/** 以 box 为中心整体缩放 factor 倍——回弹靠这个和主尺寸补间接在一起。 */
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

function fadeOnly(el: Element, done: () => void, dir: "in" | "out") {
  const frames = dir === "in" ? [{ opacity: 0 }, { opacity: 1 }] : [{ opacity: 1 }, { opacity: 0 }];
  const anim = (el as HTMLElement).animate(frames, {
    duration: prefersReducedMotion ? 1 : dir === "in" ? 160 : 140,
    easing: "ease",
  });
  anim.onfinish = done;
}

function onEnter(el: Element, done: () => void) {
  const card = cardRef.value;
  const inner = innerRef.value;
  const originEl = props.origin;
  if (props.animate === false || prefersReducedMotion || !card || !inner || !originEl) {
    fadeOnly(el, done, "in");
    return;
  }

  const from = boxOf(originEl);
  const to = boxOf(card);
  const travelPoint = centeredBox(from.w, from.h, to);

  originEl.style.visibility = "hidden";
  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, props.fromRadius));
  inner.style.opacity = "0";

  const duration = 480;
  const TRAVEL = 0.3;
  const UNFURL = 0.68;
  const OVERSHOOT = 0.8;
  const r0 = props.fromRadius;
  const r1 = props.toRadius;

  const boxAnim = card.animate(
    [
      { ...boxStyle(from, r0), offset: 0 },
      { ...boxStyle(travelPoint, r0), offset: TRAVEL },
      { ...boxStyle(to, r1), offset: UNFURL },
      { ...boxStyle(scaledBox(to, 1.03), r1 + 1), offset: OVERSHOOT },
      { ...boxStyle(scaledBox(to, 0.994), r1), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.4 },
      { ...boxStyle(scaledBox(to, 1.003), r1), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.68 },
      { ...boxStyle(to, r1), offset: 1 },
    ],
    { duration, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "forwards" },
  );

  const contentFrames = [
    { opacity: 0, offset: 0 },
    { opacity: 0, offset: TRAVEL },
    { opacity: 1, offset: UNFURL },
    { opacity: 1, offset: 1 },
  ];
  inner.animate(contentFrames, { duration, easing: "ease-out", fill: "forwards" });
  (el as HTMLElement).animate(contentFrames, { duration, fill: "forwards" });

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
  if (props.animate === false || prefersReducedMotion || !card || !inner || !originEl) {
    fadeOnly(el, done, "out");
    return;
  }

  const from = boxOf(card);
  const to = boxOf(originEl);
  const shrinkPoint = centeredBox(to.w, to.h, from);

  card.style.position = "fixed";
  card.style.margin = "0";
  card.style.overflow = "hidden";
  Object.assign(card.style, boxStyle(from, props.toRadius));

  const duration = 430;
  const SHRINK = 0.3;

  const boxAnim = card.animate(
    [
      { ...boxStyle(from, props.toRadius), offset: 0 },
      { ...boxStyle(shrinkPoint, props.fromRadius), offset: SHRINK },
      { ...boxStyle(to, props.fromRadius), offset: 1 },
    ],
    { duration, easing: "cubic-bezier(0.4,0,0.2,1)", fill: "forwards" },
  );

  const contentFrames = [
    { opacity: 1, offset: 0 },
    { opacity: 0, offset: SHRINK * 0.6 },
    { opacity: 0, offset: 1 },
  ];
  inner.animate(contentFrames, { duration, easing: "ease-in", fill: "forwards" });
  (el as HTMLElement).animate(contentFrames, { duration, fill: "forwards" });

  boxAnim.onfinish = () => {
    originEl.style.visibility = "";
    done();
  };
}

/** Esc 关闭是所有弹窗的通用行为，统一放在外壳里，各弹窗不必各自实现。 */
function onEscape(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}

watch(open, (isOpen) => {
  if (isOpen) window.addEventListener("keydown", onEscape);
  else window.removeEventListener("keydown", onEscape);
});

onBeforeUnmount(() => window.removeEventListener("keydown", onEscape));

function onBackdrop(event: MouseEvent, kind: "click" | "mousedown") {
  if (props.closeOn !== kind) return;
  if (event.target !== event.currentTarget) return;
  open.value = false;
}
</script>

<template>
  <Teleport to="body">
    <Transition :css="false" @enter="onEnter" @leave="onLeave">
      <div
        v-if="open"
        class="modal-overlay"
        @click="onBackdrop($event, 'click')"
        @mousedown="onBackdrop($event, 'mousedown')"
      >
        <div ref="cardRef" :class="['modal-card', props.cardClass]">
          <div ref="innerRef" :class="['modal-inner', props.innerClass]"><slot /></div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
