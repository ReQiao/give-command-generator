<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { CatalogRow } from "../data/catalog";
import { matches } from "../logic/builder";

const props = withDefaults(
  defineProps<{
    catalog: readonly CatalogRow[];
    placeholder?: string;
    limit?: number;
    explain?: boolean;
  }>(),
  {
    placeholder: "",
    limit: 60,
    explain: false,
  },
);

const model = defineModel<string>({ required: true });
const open = ref(false);
const activeIndex = ref(0);
const root = ref<HTMLElement | null>(null);
const input = ref<HTMLInputElement | null>(null);
const menu = ref<HTMLElement | null>(null);
const menuStyle = ref<Record<string, string>>({});

const suggestions = computed(() => {
  const query = model.value.trim();
  const rows = query ? props.catalog.filter((row) => matches(row, query)) : [...props.catalog];
  return rows.slice(0, props.limit);
});

watch(suggestions, () => {
  activeIndex.value = 0;
  if (open.value) void updateMenuPosition();
});

function display(row: CatalogRow): string {
  return String(row[1]);
}

function meta(row: CatalogRow): string {
  return String(row[0]);
}

function detail(row: CatalogRow): string {
  if (!props.explain) return "";
  const value = row[3] ?? row[2] ?? "";
  return typeof value === "string" ? value : "";
}

function choose(row: CatalogRow) {
  model.value = display(row);
  open.value = false;
}

function completeFirst() {
  const row = suggestions.value[activeIndex.value] ?? suggestions.value[0];
  if (row) choose(row);
}

async function move(delta: number) {
  open.value = true;
  if (!suggestions.value.length) return;
  activeIndex.value = (activeIndex.value + delta + suggestions.value.length) % suggestions.value.length;
  await updateMenuPosition();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Tab" && suggestions.value.length) {
    event.preventDefault();
    completeFirst();
  } else if (event.key === "Enter" && open.value && suggestions.value.length) {
    event.preventDefault();
    completeFirst();
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    move(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    move(-1);
  } else if (event.key === "Escape") {
    open.value = false;
  }
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target as Node;
  if (!root.value?.contains(target) && !menu.value?.contains(target)) open.value = false;
}

async function openMenu() {
  open.value = true;
  await updateMenuPosition();
}

async function updateMenuPosition() {
  await nextTick();
  const rect = input.value?.getBoundingClientRect();
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
  <div ref="root" class="catalog-combo" :class="{ open }">
    <input
      ref="input"
      v-model="model"
      :placeholder="props.placeholder"
      @focus="openMenu"
      @input="openMenu"
      @keydown="onKeydown"
    />

    <Teleport to="body">
      <Transition name="combo-menu">
        <div v-if="open && suggestions.length" ref="menu" class="combo-menu catalog-menu floating-menu" :style="menuStyle">
          <button
            v-for="(row, index) in suggestions"
            :key="meta(row)"
            :class="{ active: model === display(row), hover: activeIndex === index }"
            :data-tip="detail(row)"
            type="button"
            @mouseenter="activeIndex = index"
            @click="choose(row)"
          >
            <span class="catalog-main">
              <span>{{ display(row) }}</span>
              <em v-if="detail(row)">{{ detail(row) }}</em>
            </span>
            <small>{{ meta(row) }}</small>
          </button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>
