<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { CatalogRow } from "../data/catalog";
import { matches } from "../logic/builder";
import ModalShell from "./ModalShell.vue";

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
</script>

<template>
  <ModalShell
    v-model:open="open"
    :animate="props.animate"
    :origin="props.origin"
    card-class="picker-card"
    inner-class="picker-inner"
    :to-radius="26"
  >
            <div class="picker-head">
              <h2>{{ props.title }}</h2>
              <button class="picker-close" type="button" aria-label="关闭" @click="open = false">×</button>
            </div>

            <input
              ref="searchInput"
              v-model="query"
              class="picker-search"
              placeholder="搜索中文名、英文 ID 或分类"
              @keydown.enter.prevent="shown.length && choose(shown[0])"
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
  </ModalShell>
</template>
