<script setup lang="ts">
import { computed, ref } from "vue";
import { EFFECTS, EFFECT_TYPES } from "../data/catalog";
import {
  fmtNumber,
  mapCatalog,
  pairText,
  pairValue,
  type EffectGroup,
  type EffectItem,
} from "../logic/builder";

defineProps<{
  title: string;
}>();

const emit = defineEmits<{
  toast: [message: string];
}>();

const model = defineModel<EffectGroup[]>({ required: true });

const selectedGroup = ref(-1);
const groupType = ref("给予状态效果");
const groupProbability = ref(100);
const groupDiameter = ref(16);
const effect = ref("速度");
const duration = ref(300);
const amplifier = ref(0);
const showParticles = ref("否");
const showIcon = ref("否");

const activeGroup = computed(() => model.value[selectedGroup.value]);

function addGroup() {
  const type = pairValue(EFFECT_TYPES, groupType.value) as EffectGroup["type"];
  const group: EffectGroup = { type };
  if (type === "apply_effects") {
    group.probability_percent = groupProbability.value;
    group.effects = [];
  } else if (type === "remove_effects") {
    group.effects = [];
  } else if (type === "teleport_randomly") {
    group.diameter = groupDiameter.value;
  }
  model.value.push(group);
  selectedGroup.value = model.value.length - 1;
}

function removeGroup() {
  if (selectedGroup.value < 0) return;
  model.value.splice(selectedGroup.value, 1);
  selectedGroup.value = Math.min(selectedGroup.value, model.value.length - 1);
}

function addSub() {
  const group = activeGroup.value;
  if (!group || !["apply_effects", "remove_effects"].includes(group.type)) {
    emit("toast", "请先选择可添加内容的效果组");
    return;
  }
  if (!group.effects) group.effects = [];
  if (group.type === "apply_effects") {
    group.effects.push({
      id: mapCatalog(EFFECTS, effect.value),
      duration: duration.value,
      amplifier: amplifier.value,
      show_particles: showParticles.value !== "否",
      show_icon: showIcon.value !== "否",
    });
  } else {
    group.effects.push(mapCatalog(EFFECTS, effect.value));
  }
}

function removeSub(index: number) {
  const group = activeGroup.value;
  if (!group?.effects) return;
  group.effects.splice(index, 1);
}

function summary(group: EffectGroup): string {
  if (group.type === "apply_effects") return `${group.effects?.length ?? 0} 个状态效果`;
  if (group.type === "remove_effects") return `移除 ${group.effects?.length ?? 0} 个状态效果`;
  if (group.type === "clear_all_effects") return "清除全部效果";
  if (group.type === "teleport_randomly") return `直径 ${fmtNumber(group.diameter ?? 16)}`;
  return "";
}

function effectText(value: EffectItem | string): string {
  const id = typeof value === "string" ? value : value.id;
  for (const row of EFFECTS) {
    if (id === row[0] || id === row[0].replace("minecraft:", "") || id === row[1]) return row[1];
  }
  return id;
}

function effectDuration(value: EffectItem | string): string {
  return typeof value === "string" ? "" : String(value.duration ?? "");
}

function effectAmplifier(value: EffectItem | string): string {
  return typeof value === "string" ? "" : String(value.amplifier ?? "");
}

function effectParticles(value: EffectItem | string): string {
  return typeof value === "string" ? "" : value.show_particles === false ? "否" : "是";
}

function effectIcon(value: EffectItem | string): string {
  return typeof value === "string" ? "" : value.show_icon === false ? "否" : "是";
}
</script>

<template>
  <section class="effect-editor">
    <label class="effect-title">{{ title }}</label>
    <div class="inline-row">
      <label>类型</label>
      <select v-model="groupType">
        <option v-for="row in EFFECT_TYPES" :key="row[1]">{{ row[0] }}</option>
      </select>
      <label>概率</label>
      <input v-model.number="groupProbability" max="100" min="0" step="0.01" type="number" />
      <label>直径</label>
      <input v-model.number="groupDiameter" min="0" step="0.001" type="number" />
      <button type="button" @click="addGroup">添加效果组</button>
      <button type="button" @click="removeGroup">删除效果组</button>
    </div>

    <table class="data-table effect-group-table">
      <thead>
        <tr>
          <th>类型</th>
          <th>概率</th>
          <th>直径</th>
          <th>内容</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(group, index) in model"
          :key="index"
          :class="{ selected: selectedGroup === index }"
          @click="selectedGroup = index"
        >
          <td>{{ pairText(EFFECT_TYPES, group.type) }}</td>
          <td>{{ group.type === "apply_effects" ? `${fmtNumber(group.probability_percent ?? 100)}%` : "" }}</td>
          <td>{{ group.type === "teleport_randomly" ? fmtNumber(group.diameter ?? 16) : "" }}</td>
          <td>{{ summary(group) }}</td>
        </tr>
      </tbody>
    </table>

    <label>选中效果组内容</label>
    <div class="inline-row">
      <label>状态效果</label>
      <input v-model="effect" list="effect-options" />
      <datalist id="effect-options">
        <option v-for="row in EFFECTS" :key="row[0]" :value="row[1]"></option>
      </datalist>
      <label>持续时间</label>
      <input v-model.number="duration" min="0" type="number" />
      <label>等级</label>
      <input v-model.number="amplifier" min="0" type="number" />
      <label>显示粒子</label>
      <select v-model="showParticles">
        <option>是</option>
        <option>否</option>
      </select>
      <label>显示图标</label>
      <select v-model="showIcon">
        <option>是</option>
        <option>否</option>
      </select>
      <button type="button" @click="addSub">添加内容</button>
    </div>

    <table class="data-table effect-sub-table">
      <thead>
        <tr>
          <th>状态效果</th>
          <th>持续时间</th>
          <th>等级</th>
          <th>显示粒子</th>
          <th>显示图标</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(item, index) in activeGroup?.effects || []" :key="index">
          <td>{{ effectText(item) }}</td>
          <td>{{ effectDuration(item) }}</td>
          <td>{{ effectAmplifier(item) }}</td>
          <td>{{ effectParticles(item) }}</td>
          <td>{{ effectIcon(item) }}</td>
          <td><button class="table-btn" type="button" @click="removeSub(index)">删除</button></td>
        </tr>
      </tbody>
    </table>
  </section>
</template>
