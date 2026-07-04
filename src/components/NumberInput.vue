<script setup lang="ts">
import { computed, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    min?: number;
    max?: number;
    step?: number;
  }>(),
  {
    min: Number.NEGATIVE_INFINITY,
    max: Number.POSITIVE_INFINITY,
    step: 1,
  },
);

const model = defineModel<number>({ required: true });
const text = ref(String(model.value ?? 0));

const stepDecimals = computed(() => {
  const value = String(props.step);
  return value.includes(".") ? value.split(".")[1].length : 0;
});

watch(model, (value) => {
  if (Number(text.value) !== value) text.value = String(value ?? 0);
});

function commit() {
  const parsed = Number(text.value);
  const next = Number.isFinite(parsed) ? clamp(parsed) : clamp(0);
  model.value = normalizePrecision(next);
  text.value = String(model.value);
}

function nudge(direction: 1 | -1) {
  const base = Number.isFinite(Number(text.value)) ? Number(text.value) : model.value;
  const next = clamp(base + props.step * direction);
  model.value = normalizePrecision(next);
  text.value = String(model.value);
}

function clamp(value: number): number {
  return Math.min(props.max, Math.max(props.min, value));
}

function normalizePrecision(value: number): number {
  if (stepDecimals.value <= 0) return Math.round(value);
  return Number(value.toFixed(stepDecimals.value));
}
</script>

<template>
  <div class="number-input">
    <input
      v-model="text"
      inputmode="decimal"
      type="text"
      @blur="commit"
      @keydown.enter.prevent="commit"
      @keydown.down.prevent="nudge(-1)"
      @keydown.up.prevent="nudge(1)"
    />
    <div class="number-steps" aria-hidden="true">
      <button tabindex="-1" type="button" @click="nudge(1)">+</button>
      <button tabindex="-1" type="button" @click="nudge(-1)">−</button>
    </div>
  </div>
</template>
