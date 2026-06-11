<script setup lang="ts">
import { computed, ref } from "vue";

defineProps<{
  text: string;
}>();

const visible = ref(false);
const anchor = ref<HTMLElement | null>(null);
type VerticalPlacement = "top" | "bottom";
type HorizontalPlacement = "left" | "center" | "right";

const position = ref({
  left: 0,
  top: 0,
  vertical: "bottom" as VerticalPlacement,
  horizontal: "center" as HorizontalPlacement,
});

const bubbleStyle = computed(() => ({
  left: `${position.value.left}px`,
  top: `${position.value.top}px`,
}));

function show() {
  const rect = anchor.value?.getBoundingClientRect();
  if (!rect) return;

  const viewportGap = 12;
  const bubbleWidth = 280;
  const placeAbove = rect.bottom + 96 > window.innerHeight && rect.top > 110;
  const center = rect.left + rect.width / 2;
  const horizontal: HorizontalPlacement =
    center - bubbleWidth / 2 < viewportGap
      ? "right"
      : center + bubbleWidth / 2 > window.innerWidth - viewportGap
        ? "left"
        : "center";

  const left =
    horizontal === "right"
      ? Math.min(rect.right + 10, window.innerWidth - viewportGap - bubbleWidth)
      : horizontal === "left"
        ? Math.max(rect.left - 10, viewportGap + bubbleWidth)
        : center;

  position.value = {
    left,
    top: placeAbove ? rect.top - 9 : rect.bottom + 9,
    vertical: placeAbove ? "top" : "bottom",
    horizontal,
  };
  visible.value = true;
}

function hide() {
  visible.value = false;
}
</script>

<template>
  <span
    ref="anchor"
    class="info-tip"
    tabindex="0"
    aria-label="说明"
    @mouseenter="show"
    @mouseleave="hide"
    @focus="show"
    @blur="hide"
  >
    ?
  </span>

  <Teleport to="body">
    <Transition name="tip-pop">
      <span v-if="visible" :class="['info-bubble', position.vertical, position.horizontal]" :style="bubbleStyle">
        {{ text }}
      </span>
    </Transition>
  </Teleport>
</template>
