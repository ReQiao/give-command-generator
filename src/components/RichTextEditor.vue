<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import CustomSelect from "./CustomSelect.vue";
import NumberInput from "./NumberInput.vue";
import {
  colorLerp,
  shadowColorInt,
  type RichLine,
  type TextComponent,
} from "../logic/builder";

interface SelectOption {
  label: string;
  value: string;
}

const props = defineProps<{
  title: string;
  multiline?: boolean;
}>();

const emit = defineEmits<{
  toast: [message: string];
}>();

const model = defineModel<RichLine[]>({ required: true });

const editor = ref<HTMLDivElement | null>(null);
const modalOpen = ref(false);
const activeTab = ref<"text" | "shadow">("text");
const textMode = ref("单色");
const textStart = ref("#000599");
const textEnd = ref("#1ad9ff");
const shadowMode = ref("跟随文字颜色");
const shadowStart = ref("#000599");
const shadowEnd = ref("#1ad9ff");
const shadowAlpha = ref(50);
const toolbarPulse = ref("");
let savedRange: Range | null = null;
let lastSnapshot = "";

const presets = [
  "#ffffff",
  "#000000",
  "#ff5555",
  "#ffaa00",
  "#ffff55",
  "#55ff55",
  "#55ffff",
  "#5555ff",
  "#aa55ff",
  "#ff55ff",
  "#000599",
  "#1ad9ff",
  "#7aa2ff",
  "#4de4c9",
];
const textModeOptions: SelectOption[] = [
  { label: "单色", value: "单色" },
  { label: "渐变", value: "渐变" },
];
const shadowModeOptions: SelectOption[] = [
  { label: "关闭", value: "关闭" },
  { label: "固定颜色", value: "固定颜色" },
  { label: "跟随文字颜色", value: "跟随文字颜色" },
  { label: "独立渐变", value: "独立渐变" },
];

onMounted(() => {
  renderFromModel();
});

watch(
  model,
  () => {
    const snapshot = JSON.stringify(model.value);
    if (snapshot !== lastSnapshot && document.activeElement !== editor.value) {
      renderFromModel();
    }
  },
  { deep: true },
);

function runCommand(command: string, label: string) {
  focusEditor();
  document.execCommand(command, false);
  updateModel();
  pulse(label);
}

function clearFormat() {
  focusEditor();
  document.execCommand("removeFormat", false);
  updateModel();
  pulse("清除格式");
}

function openColor(tab: "text" | "shadow") {
  if (!storeRange()) {
    emit("toast", "请先选中文字");
    return;
  }
  activeTab.value = tab;
  modalOpen.value = true;
}

function applyColor() {
  const range = restoreRange();
  if (!range || range.collapsed) {
    emit("toast", "请先选中文字");
    return;
  }

  const text = range.toString();
  const count = text.length;
  if (!count) return;

  const textColors = textMode.value === "单色"
    ? Array.from({ length: count }, () => textStart.value)
    : colorLerp(textStart.value, textEnd.value, count);
  const shadowColors = shadowMode.value === "独立渐变"
    ? colorLerp(shadowStart.value, shadowEnd.value, count)
    : Array.from({ length: count }, (_, index) => shadowMode.value === "跟随文字颜色" ? textColors[index] : shadowStart.value);

  const fragment = document.createDocumentFragment();
  Array.from(text).forEach((char, index) => {
    if (char === "\n") {
      fragment.appendChild(document.createElement("br"));
      return;
    }

    const span = document.createElement("span");
    span.textContent = char;

    if (activeTab.value === "text") {
      span.style.color = textColors[index];
      span.dataset.color = textColors[index];
    }

    if (shadowMode.value !== "关闭") {
      const shadowValue = shadowColorInt(shadowColors[index], shadowAlpha.value);
      span.dataset.shadow = String(shadowValue);
      span.style.textShadow = `0 0 5px ${hexToRgba(shadowColors[index], shadowAlpha.value / 100)}`;
    }

    fragment.appendChild(span);
  });

  range.deleteContents();
  range.insertNode(fragment);
  modalOpen.value = false;
  savedRange = null;
  updateModel();
  pulse(activeTab.value === "text" ? "颜色" : "阴影");
}

function updateModel() {
  const lines = serializeEditor();
  lastSnapshot = JSON.stringify(lines);
  model.value = lines;
}

function renderFromModel() {
  if (!editor.value) return;
  editor.value.innerHTML = linesToHtml(model.value);
  lastSnapshot = JSON.stringify(model.value);
}

function serializeEditor(): RichLine[] {
  const host = editor.value;
  if (!host) return [];
  const lines: RichLine[] = [[]];

  const pushLine = () => {
    if (lines[lines.length - 1].length) {
      lines.push([]);
    }
  };

  const pushText = (text: string, context: Partial<TextComponent>) => {
    for (const char of text) {
      if (char === "\n") {
        pushLine();
        continue;
      }
      const item: TextComponent = { text: char };
      if (context.bold) item.bold = true;
      if (context.italic) item.italic = true;
      if (context.underlined) item.underlined = true;
      if (context.strikethrough) item.strikethrough = true;
      if (context.color) item.color = context.color;
      if (context.shadow_color !== undefined) item.shadow_color = context.shadow_color;
      lines[lines.length - 1].push(item);
    }
  };

  const walk = (node: Node, context: Partial<TextComponent>) => {
    if (node.nodeType === Node.TEXT_NODE) {
      pushText(node.textContent ?? "", context);
      return;
    }

    if (!(node instanceof HTMLElement)) return;
    if (node.tagName === "BR") {
      pushLine();
      return;
    }

    const next = { ...context };
    const tag = node.tagName.toLowerCase();
    const style = window.getComputedStyle(node);
    if (tag === "b" || tag === "strong" || Number.parseInt(style.fontWeight, 10) >= 700) next.bold = true;
    if (tag === "i" || tag === "em" || style.fontStyle === "italic") next.italic = true;
    if (tag === "u" || style.textDecorationLine.includes("underline")) next.underlined = true;
    if (tag === "s" || tag === "strike" || style.textDecorationLine.includes("line-through")) next.strikethrough = true;
    if (node.dataset.color) next.color = node.dataset.color;
    if (node.dataset.shadow) next.shadow_color = Number(node.dataset.shadow);

    node.childNodes.forEach((child) => walk(child, next));
    if (["div", "p"].includes(tag)) pushLine();
  };

  host.childNodes.forEach((child) => walk(child, {}));
  return lines.filter((line) => line.length);
}

function linesToHtml(lines: RichLine[]): string {
  return lines
    .map((line) => line.map(componentToHtml).join(""))
    .join("<br>");
}

function componentToHtml(item: TextComponent): string {
  const styles: string[] = [];
  const attrs: string[] = [];
  let text = escapeHtml(item.text);
  if (item.color) {
    styles.push(`color:${item.color}`);
    attrs.push(`data-color="${item.color}"`);
  }
  if (item.shadow_color !== undefined) {
    attrs.push(`data-shadow="${item.shadow_color}"`);
    styles.push("text-shadow:0 0 5px rgba(122,162,255,.55)");
  }
  if (item.bold) text = `<strong>${text}</strong>`;
  if (item.italic) text = `<em>${text}</em>`;
  if (item.underlined) text = `<u>${text}</u>`;
  if (item.strikethrough) text = `<s>${text}</s>`;
  return `<span ${attrs.join(" ")} style="${styles.join(";")}">${text}</span>`;
}

function storeRange(): boolean {
  const selection = window.getSelection();
  const host = editor.value;
  if (!selection || !host || selection.rangeCount === 0) return false;
  const range = selection.getRangeAt(0);
  if (!host.contains(range.commonAncestorContainer) || range.collapsed) return false;
  savedRange = range.cloneRange();
  return true;
}

function restoreRange(): Range | null {
  if (!savedRange) return null;
  const selection = window.getSelection();
  if (!selection) return savedRange;
  selection.removeAllRanges();
  selection.addRange(savedRange);
  return savedRange;
}

function focusEditor() {
  editor.value?.focus();
  nextTick(updateModel);
}

function pulse(label: string) {
  toolbarPulse.value = label;
  window.setTimeout(() => {
    if (toolbarPulse.value === label) toolbarPulse.value = "";
  }, 360);
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function hexToRgba(hex: string, alpha: number): string {
  const r = Number.parseInt(hex.slice(1, 3), 16);
  const g = Number.parseInt(hex.slice(3, 5), 16);
  const b = Number.parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
</script>

<template>
  <section class="rich-editor">
    <label>{{ props.title }}</label>
    <div class="rich-toolbar">
      <button :class="{ pulse: toolbarPulse === '加粗' }" type="button" @click="runCommand('bold', '加粗')">加粗</button>
      <button :class="{ pulse: toolbarPulse === '斜体' }" type="button" @click="runCommand('italic', '斜体')">斜体</button>
      <button :class="{ pulse: toolbarPulse === '下划线' }" type="button" @click="runCommand('underline', '下划线')">下划线</button>
      <button :class="{ pulse: toolbarPulse === '删除线' }" type="button" @click="runCommand('strikeThrough', '删除线')">删除线</button>
      <button :class="{ pulse: toolbarPulse === '颜色' }" type="button" @click="openColor('text')">颜色</button>
      <button :class="{ pulse: toolbarPulse === '阴影' }" type="button" @click="openColor('shadow')">阴影</button>
      <button :class="{ pulse: toolbarPulse === '清除格式' }" type="button" @click="clearFormat">清除格式</button>
    </div>
    <div
      ref="editor"
      :class="['rich-input', { multiline: props.multiline }]"
      contenteditable="true"
      spellcheck="false"
      @input="updateModel"
      @blur="updateModel"
    ></div>
  </section>

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="modalOpen" class="modal-overlay" @mousedown.self="modalOpen = false">
        <div class="modal-card color-card">
          <h2>颜色</h2>
          <div class="tab-strip compact">
            <button :class="{ active: activeTab === 'text' }" type="button" @click="activeTab = 'text'">文字颜色</button>
            <button :class="{ active: activeTab === 'shadow' }" type="button" @click="activeTab = 'shadow'">阴影颜色</button>
          </div>

          <div v-if="activeTab === 'text'" class="color-page">
            <label>模式</label>
            <CustomSelect v-model="textMode" :options="textModeOptions" />
            <label>起始颜色</label>
            <input v-model="textStart" type="color" />
            <label>结束颜色</label>
            <input v-model="textEnd" type="color" />
          </div>

          <div v-else class="color-page">
            <label>模式</label>
            <CustomSelect v-model="shadowMode" :options="shadowModeOptions" />
            <label>起始颜色</label>
            <input v-model="shadowStart" type="color" />
            <label>结束颜色</label>
            <input v-model="shadowEnd" type="color" />
            <label>透明度</label>
            <NumberInput v-model="shadowAlpha" :max="100" :min="0" />
          </div>

          <div class="preset-row">
            <button
              v-for="color in presets"
              :key="color"
              :style="{ background: color }"
              type="button"
              @click="activeTab === 'text' ? (textStart = color) : (shadowStart = color)"
            ></button>
          </div>

          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="modalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applyColor">应用</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
