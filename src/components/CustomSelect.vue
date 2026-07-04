<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";

interface SelectOption {
  label: string;
  value: string;
  description?: string;
}

const props = defineProps<{
  options: SelectOption[];
}>();

const model = defineModel<string>({ required: true });
const open = ref(false);
const activeIndex = ref(0);
const root = ref<HTMLElement | null>(null);
const trigger = ref<HTMLElement | null>(null);
const menu = ref<HTMLElement | null>(null);
const menuStyle = ref<Record<string, string>>({});

const selected = computed(() => props.options.find((option) => option.value === model.value));

function choose(option: SelectOption) {
  model.value = option.value;
  open.value = false;
}

async function toggle() {
  open.value = !open.value;
  activeIndex.value = Math.max(0, props.options.findIndex((option) => option.value === model.value));
  if (open.value) await updateMenuPosition();
}

async function move(delta: number) {
  open.value = true;
  activeIndex.value = (activeIndex.value + delta + props.options.length) % props.options.length;
  await updateMenuPosition();
}

function confirm() {
  const option = props.options[activeIndex.value];
  if (option) choose(option);
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target as Node;
  if (!root.value?.contains(target) && !menu.value?.contains(target)) open.value = false;
}

async function updateMenuPosition() {
  await nextTick();
  const rect = trigger.value?.getBoundingClientRect();
  if (!rect) return;

  const viewportGap = 12;
  const menuHeight = Math.min(280, window.innerHeight * 0.42);
  const spaceBelow = window.innerHeight - rect.bottom - viewportGap;
  const openAbove = spaceBelow < Math.min(180, menuHeight) && rect.top > spaceBelow;
  const top = openAbove ? Math.max(viewportGap, rect.top - menuHeight - 6) : Math.min(rect.bottom + 6, window.innerHeight - viewportGap);

  menuStyle.value = {
    left: `${rect.left}px`,
    top: `${top}px`,
    width: `${rect.width}px`,
    maxHeight: `${openAbove ? Math.min(menuHeight, rect.top - viewportGap - 6) : Math.min(menuHeight, spaceBelow - 6)}px`,
  };
}

document.addEventListener("pointerdown", onDocumentPointerDown);
window.addEventListener("resize", updateMenuPosition);
window.addEventListener("scroll", updateMenuPosition, true);

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown);
  window.removeEventListener("resize", updateMenuPosition);
  window.removeEventListener("scroll", updateMenuPosition, true);
});
</script>

<template>
  <div ref="root" class="custom-select" :class="{ open }">
    <button
      ref="trigger"
      class="custom-select-trigger"
      type="button"
      @click="toggle"
      @keydown.down.prevent="move(1)"
      @keydown.up.prevent="move(-1)"
      @keydown.enter.prevent="confirm"
      @keydown.esc.prevent="open = false"
    >
      <span>{{ selected?.label ?? model }}</span>
      <span class="menu-mark">+</span>
    </button>

    <Teleport to="body">
      <Transition name="combo-menu">
        <div v-if="open" ref="menu" class="combo-menu select-menu floating-menu" :style="menuStyle">
          <button
            v-for="(option, index) in props.options"
            :key="option.value"
            :class="{ active: option.value === model, hover: activeIndex === index }"
            type="button"
            @mouseenter="activeIndex = index"
            @click="choose(option)"
          >
            <span>{{ option.label }}</span>
            <small v-if="option.description">{{ option.description }}</small>
          </button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>
