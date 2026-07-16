<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import CatalogCombo from "./CatalogCombo.vue";
import CustomSelect from "./CustomSelect.vue";
import NumberInput from "./NumberInput.vue";
import { BLOCKS, ITEMS } from "../data/catalog";
import {
  colorLerp,
  mapCatalog,
  resolveTextProfile,
  shadowColorInt,
  stripMinecraftNamespace,
  type GiveVersion,
  type RichComponent,
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
  version?: string;
}>();

const supportsObject = computed(() =>
  props.version ? resolveTextProfile(props.version as GiveVersion).supportsObjectComponent : true,
);

const emit = defineEmits<{
  toast: [message: string];
}>();

const model = defineModel<RichLine[]>({ required: true });

const editor = ref<HTMLDivElement | null>(null);
const modalOpen = ref(false);
const fontModalOpen = ref(false);
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

// 字体：Minecraft 内置字体 + 自定义
const fontChoice = ref("");
const fontOptions: SelectOption[] = [
  { label: "默认字体", value: "" },
  { label: "统一字体 uniform", value: "minecraft:uniform" },
  { label: "标准银河字母 alt", value: "minecraft:alt" },
  { label: "illager 符文 illageralt", value: "minecraft:illageralt" },
  { label: "自定义…", value: "__custom__" },
];
const customFont = ref("");

// object 组件插入（1.21.9+）
const spriteModalOpen = ref(false);
const playerModalOpen = ref(false);
const spriteKind = ref("block");
const spritePick = ref("");
const spritePath = ref("block/stone");
const spriteAtlas = ref("minecraft:blocks");
const spriteKindOptions: SelectOption[] = [
  { label: "方块贴图 block/", value: "block" },
  { label: "物品贴图 item/", value: "item" },
];
const playerName = ref("");
const playerHat = ref(true);

// 交互事件：insertion / click_event / hover_event
const eventModalOpen = ref(false);
const insertionText = ref("");
const clickAction = ref("");
const clickValue = ref("");
const hoverAction = ref("");
const hoverText = ref("");
const hoverItemId = ref("minecraft:stone");
const hoverItemCount = ref(1);
const hoverEntityType = ref("minecraft:pig");
const hoverEntityUuid = ref("");
const hoverEntityName = ref("");
const clickActionOptions: SelectOption[] = [
  { label: "无", value: "" },
  { label: "打开网址 open_url", value: "open_url" },
  { label: "运行命令 run_command", value: "run_command" },
  { label: "填入命令 suggest_command", value: "suggest_command" },
  { label: "复制到剪贴板 copy_to_clipboard", value: "copy_to_clipboard" },
  { label: "翻书页 change_page", value: "change_page" },
  { label: "打开对话框 show_dialog", value: "show_dialog" },
];
const hoverActionOptions: SelectOption[] = [
  { label: "无", value: "" },
  { label: "显示文本 show_text", value: "show_text" },
  { label: "显示物品 show_item", value: "show_item" },
  { label: "显示实体 show_entity", value: "show_entity" },
];
const clickValueLabel = computed(() => {
  switch (clickAction.value) {
    case "open_url": return "网址";
    case "run_command": case "suggest_command": return "命令";
    case "copy_to_clipboard": return "文本";
    case "change_page": return "页码";
    case "show_dialog": return "对话框 ID";
    default: return "值";
  }
});

// 高级内容类型：translatable / keybind / selector / score / nbt
const compModalOpen = ref(false);
const compType = ref("translatable");
const compTypeOptions: SelectOption[] = [
  { label: "翻译文本 translatable", value: "translatable" },
  { label: "按键 keybind", value: "keybind" },
  { label: "选择器 selector", value: "selector" },
  { label: "计分板 score", value: "score" },
  { label: "NBT 读取 nbt", value: "nbt" },
];
const transKey = ref("");
const transFallback = ref("");
const transWith = ref("");
const keybindKey = ref("key.jump");
const selectorValue = ref("@p");
const selectorSep = ref("");
const scoreName = ref("@s");
const scoreObjective = ref("");
const nbtPath = ref("");
const nbtSource = ref("entity");
const nbtTarget = ref("@s");
const nbtInterpret = ref(false);
const nbtSourceOptions: SelectOption[] = [
  { label: "实体 entity", value: "entity" },
  { label: "方块 block", value: "block" },
  { label: "存储 storage", value: "storage" },
];
const nbtTargetLabel = computed(() => {
  switch (nbtSource.value) {
    case "block": return "方块坐标（如 ~ ~ ~）";
    case "storage": return "存储 ID（如 my:data）";
    default: return "实体选择器（如 @s）";
  }
});

watch([spriteKind, spritePick], () => {
  const id = spritePick.value.trim();
  if (!id) return;
  const catalog = spriteKind.value === "block" ? BLOCKS : ITEMS;
  const resolved = stripMinecraftNamespace(mapCatalog(catalog, id));
  spritePath.value = `${spriteKind.value}/${resolved}`;
});

// 命名颜色（值为 MC 颜色名，展示用近似 hex）
const namedColors: { name: string; hex: string }[] = [
  { name: "black", hex: "#000000" },
  { name: "dark_blue", hex: "#0000aa" },
  { name: "dark_green", hex: "#00aa00" },
  { name: "dark_aqua", hex: "#00aaaa" },
  { name: "dark_red", hex: "#aa0000" },
  { name: "dark_purple", hex: "#aa00aa" },
  { name: "gold", hex: "#ffaa00" },
  { name: "gray", hex: "#aaaaaa" },
  { name: "dark_gray", hex: "#555555" },
  { name: "blue", hex: "#5555ff" },
  { name: "green", hex: "#55ff55" },
  { name: "aqua", hex: "#55ffff" },
  { name: "red", hex: "#ff5555" },
  { name: "light_purple", hex: "#ff55ff" },
  { name: "yellow", hex: "#ffff55" },
  { name: "white", hex: "#ffffff" },
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

function currentRange(): Range | null {
  const selection = window.getSelection();
  const host = editor.value;
  if (!selection || !host || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  if (!host.contains(range.commonAncestorContainer) || range.collapsed) return null;
  return range;
}

// 用一个 span 包住选区（保留内部加粗/斜体等结构），并回调设置该 span。
function wrapRange(range: Range, setup: (span: HTMLSpanElement) => void) {
  const span = document.createElement("span");
  span.appendChild(range.extractContents());
  setup(span);
  range.insertNode(span);
}

function wrapSelection(setup: (span: HTMLSpanElement) => void, label: string) {
  const range = currentRange();
  if (!range) {
    emit("toast", "请先选中文字");
    return;
  }
  wrapRange(range, setup);
  editor.value?.focus();
  updateModel();
  pulse(label);
}

function openFont() {
  if (!storeRange()) {
    emit("toast", "请先选中文字");
    return;
  }
  fontModalOpen.value = true;
}

function applyFont() {
  const range = restoreRange();
  if (!range || range.collapsed) {
    emit("toast", "请先选中文字");
    fontModalOpen.value = false;
    return;
  }
  const font = fontChoice.value === "__custom__" ? customFont.value.trim() : fontChoice.value;
  wrapRange(range, (span) => {
    // 去掉内部已有 font 标记，让本次选择统一生效（“默认字体”则清空）
    span.querySelectorAll<HTMLElement>("[data-font]").forEach((el) => {
      delete el.dataset.font;
    });
    if (font) span.dataset.font = font;
  });
  fontModalOpen.value = false;
  savedRange = null;
  updateModel();
  pulse("字体");
}

// 记录当前光标（允许折叠位置，用于插入原子芯片）
function storeCaret(): boolean {
  const selection = window.getSelection();
  const host = editor.value;
  if (!selection || !host || selection.rangeCount === 0) return false;
  const range = selection.getRangeAt(0);
  if (!host.contains(range.commonAncestorContainer)) return false;
  savedRange = range.cloneRange();
  return true;
}

function insertAtom(run: RichComponent) {
  const host = editor.value;
  if (!host) return;
  host.focus();
  let range = restoreRange();
  if (!range || !host.contains(range.commonAncestorContainer)) {
    range = document.createRange();
    range.selectNodeContents(host);
    range.collapse(false);
  }
  range.deleteContents();
  const template = document.createElement("template");
  template.innerHTML = atomToHtml(run);
  const node = template.content.firstChild;
  if (!node) return;
  range.insertNode(node);
  // 补一个零宽字符并把光标移到芯片后（序列化时会被忽略）
  const filler = document.createTextNode("​");
  (node as ChildNode).after(filler);
  const after = document.createRange();
  after.setStartAfter(filler);
  after.collapse(true);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(after);
  savedRange = null;
  updateModel();
}

function openSprite() {
  if (!supportsObject.value) {
    emit("toast", "内嵌图标需要 Java 1.21.9+");
    return;
  }
  storeCaret();
  spriteModalOpen.value = true;
}

function applySprite() {
  const sprite = spritePath.value.trim();
  if (!sprite) {
    emit("toast", "请填写贴图路径");
    return;
  }
  insertAtom({
    type: "object",
    object: "atlas",
    atlas: spriteAtlas.value.trim() || "minecraft:blocks",
    sprite,
  });
  spriteModalOpen.value = false;
  pulse("图标");
}

function openPlayer() {
  if (!supportsObject.value) {
    emit("toast", "内嵌头像需要 Java 1.21.9+");
    return;
  }
  storeCaret();
  playerModalOpen.value = true;
}

function applyPlayer() {
  const name = playerName.value.trim();
  if (!name) {
    emit("toast", "请填写玩家名");
    return;
  }
  insertAtom({
    type: "object",
    object: "player",
    player: name,
    hat: playerHat.value,
  });
  playerModalOpen.value = false;
  pulse("头像");
}

function openComponent() {
  storeCaret();
  compModalOpen.value = true;
}

function buildAdvancedComponent(): RichComponent | null {
  const t = compType.value;
  if (t === "translatable") {
    const key = transKey.value.trim();
    if (!key) return null;
    const run: Record<string, unknown> = { type: "translatable", translate: key };
    if (transFallback.value.trim()) run.fallback = transFallback.value.trim();
    const args = transWith.value.split("|").map((s) => s.trim()).filter(Boolean);
    if (args.length) run.with = args.map((a) => ({ text: a }));
    return run as unknown as RichComponent;
  }
  if (t === "keybind") {
    const key = keybindKey.value.trim();
    if (!key) return null;
    return { type: "keybind", keybind: key } as unknown as RichComponent;
  }
  if (t === "selector") {
    const sel = selectorValue.value.trim();
    if (!sel) return null;
    const run: Record<string, unknown> = { type: "selector", selector: sel };
    if (selectorSep.value.trim()) run.separator = { text: selectorSep.value };
    return run as unknown as RichComponent;
  }
  if (t === "score") {
    const name = scoreName.value.trim();
    const objective = scoreObjective.value.trim();
    if (!name || !objective) return null;
    return { type: "score", score: { name, objective } } as unknown as RichComponent;
  }
  // nbt
  const path = nbtPath.value.trim();
  const target = nbtTarget.value.trim();
  if (!path || !target) return null;
  const run: Record<string, unknown> = { type: "nbt", nbt: path, source: nbtSource.value };
  if (nbtSource.value === "block") run.block = target;
  else if (nbtSource.value === "storage") run.storage = target;
  else run.entity = target;
  if (nbtInterpret.value) run.interpret = true;
  return run as unknown as RichComponent;
}

function applyComponent() {
  const run = buildAdvancedComponent();
  if (!run) {
    emit("toast", "请填写必要字段");
    return;
  }
  insertAtom(run);
  compModalOpen.value = false;
  pulse("组件");
}

function openEvents() {
  if (!storeRange()) {
    emit("toast", "请先选中文字");
    return;
  }
  eventModalOpen.value = true;
}

function buildHoverEvent(): Record<string, unknown> | null {
  const action = hoverAction.value;
  if (!action) return null;
  if (action === "show_text") {
    return { action, text: [{ text: hoverText.value }] };
  }
  if (action === "show_item") {
    const ev: Record<string, unknown> = { action, itemId: hoverItemId.value.trim() };
    const count = Number(hoverItemCount.value);
    if (Number.isFinite(count) && count > 0) ev.itemCount = count;
    return ev;
  }
  // show_entity
  const ev: Record<string, unknown> = { action, entityType: hoverEntityType.value.trim() };
  if (hoverEntityUuid.value.trim()) ev.entityUuid = hoverEntityUuid.value.trim();
  if (hoverEntityName.value.trim()) ev.entityName = [{ text: hoverEntityName.value.trim() }];
  return ev;
}

function applyEvents() {
  const range = restoreRange();
  if (!range || range.collapsed) {
    emit("toast", "请先选中文字");
    eventModalOpen.value = false;
    return;
  }
  const ins = insertionText.value.trim();
  const click = clickAction.value ? { action: clickAction.value, value: clickValue.value } : null;
  const hover = buildHoverEvent();
  wrapRange(range, (span) => {
    // 清掉内部旧事件标记，让本次统一生效
    span.querySelectorAll<HTMLElement>("[data-insertion],[data-clickevent],[data-hoverevent]").forEach((el) => {
      delete el.dataset.insertion;
      delete el.dataset.clickevent;
      delete el.dataset.hoverevent;
      el.classList.remove("rt-event");
    });
    if (ins) span.dataset.insertion = ins;
    if (click) span.dataset.clickevent = JSON.stringify(click);
    if (hover) span.dataset.hoverevent = JSON.stringify(hover);
    if (ins || click || hover) span.classList.add("rt-event");
  });
  eventModalOpen.value = false;
  savedRange = null;
  updateModel();
  pulse("交互");
}

function applyNamedColor(name: string, hex: string) {
  const range = restoreRange();
  if (!range || range.collapsed) {
    emit("toast", "请先选中文字");
    modalOpen.value = false;
    return;
  }
  wrapRange(range, (span) => {
    span.querySelectorAll<HTMLElement>("[data-color]").forEach((el) => {
      delete el.dataset.color;
      el.style.color = "";
    });
    span.dataset.color = name;
    span.style.color = hex;
  });
  modalOpen.value = false;
  savedRange = null;
  updateModel();
  pulse("颜色");
}

function toggleObfuscated() {
  wrapSelection((span) => {
    const inner = span.querySelectorAll<HTMLElement>("[data-obfuscated]");
    const allObfuscated = inner.length > 0 && Array.from(inner).every((el) => el.dataset.obfuscated === "1");
    inner.forEach((el) => {
      delete el.dataset.obfuscated;
      el.classList.remove("rt-obf");
    });
    if (!allObfuscated) {
      span.dataset.obfuscated = "1";
      span.classList.add("rt-obf");
    }
  }, "混淆");
}

function clearFormat() {
  focusEditor();
  document.execCommand("removeFormat", false);
  const range = currentRange();
  if (range) {
    const span = document.createElement("span");
    span.appendChild(range.extractContents());
    span.querySelectorAll<HTMLElement>("[data-font],[data-color],[data-shadow],[data-obfuscated]").forEach((el) => {
      delete el.dataset.font;
      delete el.dataset.color;
      delete el.dataset.shadow;
      delete el.dataset.obfuscated;
      el.classList.remove("rt-obf");
      el.style.color = "";
      el.style.textShadow = "";
    });
    const frag = document.createDocumentFragment();
    while (span.firstChild) frag.appendChild(span.firstChild);
    range.insertNode(frag);
  }
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
      if (char === "​") continue; // 忽略插入芯片时的零宽占位
      if (char === "\n") {
        pushLine();
        continue;
      }
      const item: TextComponent = { text: char };
      if (context.bold) item.bold = true;
      if (context.italic) item.italic = true;
      if (context.underlined) item.underlined = true;
      if (context.strikethrough) item.strikethrough = true;
      if (context.obfuscated) item.obfuscated = true;
      if (context.color) item.color = context.color;
      if (context.font) item.font = context.font;
      if (context.shadow_color !== undefined) item.shadow_color = context.shadow_color;
      if (context.insertion) item.insertion = context.insertion;
      if (context.click_event) item.click_event = context.click_event;
      if (context.hover_event) item.hover_event = context.hover_event;
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
    // 原子芯片（object/翻译/…）：直接还原存储的运行，不再逐字递归
    if (node.dataset.run) {
      try {
        lines[lines.length - 1].push(JSON.parse(node.dataset.run) as RichComponent);
      } catch {
        // 忽略损坏的载荷
      }
      return;
    }

    const next = { ...context };
    const tag = node.tagName.toLowerCase();
    const style = window.getComputedStyle(node);
    if (tag === "b" || tag === "strong" || Number.parseInt(style.fontWeight, 10) >= 700) next.bold = true;
    if (tag === "i" || tag === "em" || style.fontStyle === "italic") next.italic = true;
    if (tag === "u" || style.textDecorationLine.includes("underline")) next.underlined = true;
    if (tag === "s" || tag === "strike" || style.textDecorationLine.includes("line-through")) next.strikethrough = true;
    if (node.dataset.obfuscated === "1") next.obfuscated = true;
    if (node.dataset.color) next.color = node.dataset.color;
    if (node.dataset.font) next.font = node.dataset.font;
    if (node.dataset.shadow) next.shadow_color = Number(node.dataset.shadow);
    if (node.dataset.insertion) next.insertion = node.dataset.insertion;
    if (node.dataset.clickevent) {
      try {
        next.click_event = JSON.parse(node.dataset.clickevent);
      } catch {
        // 忽略损坏数据
      }
    }
    if (node.dataset.hoverevent) {
      try {
        next.hover_event = JSON.parse(node.dataset.hoverevent);
      } catch {
        // 忽略损坏数据
      }
    }

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

function cssColor(color: string): string {
  const named = namedColors.find((c) => c.name === color);
  return named ? named.hex : color;
}

function componentToHtml(component: RichComponent): string {
  const runType = (component as { type?: string }).type;
  if (runType && runType !== "text") {
    return atomToHtml(component);
  }
  const item = component as TextComponent;
  const styles: string[] = [];
  const attrs: string[] = [];
  const classes: string[] = [];
  let text = escapeHtml(item.text ?? "");
  if (item.color) {
    styles.push(`color:${cssColor(item.color)}`);
    attrs.push(`data-color="${item.color}"`);
  }
  if (item.font) {
    attrs.push(`data-font="${item.font}"`);
  }
  if (item.obfuscated) {
    attrs.push(`data-obfuscated="1"`);
    classes.push("rt-obf");
  }
  if (item.shadow_color !== undefined) {
    attrs.push(`data-shadow="${item.shadow_color}"`);
    styles.push("text-shadow:0 0 5px rgba(122,162,255,.55)");
  }
  if (item.insertion) {
    attrs.push(`data-insertion="${escapeHtml(item.insertion)}"`);
    classes.push("rt-event");
  }
  if (item.click_event) {
    attrs.push(`data-clickevent="${escapeHtml(JSON.stringify(item.click_event))}"`);
    classes.push("rt-event");
  }
  if (item.hover_event) {
    attrs.push(`data-hoverevent="${escapeHtml(JSON.stringify(item.hover_event))}"`);
    classes.push("rt-event");
  }
  if (item.bold) text = `<strong>${text}</strong>`;
  if (item.italic) text = `<em>${text}</em>`;
  if (item.underlined) text = `<u>${text}</u>`;
  if (item.strikethrough) text = `<s>${text}</s>`;
  const classAttr = classes.length ? ` class="${classes.join(" ")}"` : "";
  return `<span ${attrs.join(" ")}${classAttr} style="${styles.join(";")}">${text}</span>`;
}

// 非文本运行（object/翻译/…）渲染为不可编辑的内联“芯片”，Phase 2+ 逐步完善展示。
function atomToHtml(component: RichComponent): string {
  const payload = escapeHtml(JSON.stringify(component));
  const label = escapeHtml(atomLabel(component));
  return `<span class="rt-atom" contenteditable="false" data-run="${payload}">${label}</span>`;
}

function atomLabel(component: RichComponent): string {
  const c = component as unknown as Record<string, unknown>;
  switch (c.type) {
    case "object":
      return c.object === "player" ? `👤 ${c.player ?? ""}` : `🧩 ${c.sprite ?? ""}`;
    case "translatable":
      return `🌐 ${c.translate ?? ""}`;
    case "keybind":
      return `⌨ ${c.keybind ?? ""}`;
    case "selector":
      return `@ ${c.selector ?? ""}`;
    case "score":
      return `# ${(c.score as { name?: string })?.name ?? ""}`;
    case "nbt":
      return `NBT ${c.nbt ?? ""}`;
    default:
      return "?";
  }
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
      <button :class="{ pulse: toolbarPulse === '混淆' }" type="button" @click="toggleObfuscated">混淆</button>
      <button :class="{ pulse: toolbarPulse === '字体' }" type="button" @click="openFont">字体</button>
      <button :class="{ pulse: toolbarPulse === '颜色' }" type="button" @click="openColor('text')">颜色</button>
      <button :class="{ pulse: toolbarPulse === '阴影' }" type="button" @click="openColor('shadow')">阴影</button>
      <button
        v-if="supportsObject"
        :class="{ pulse: toolbarPulse === '图标' }"
        type="button"
        title="插入内嵌方块/物品图标（1.21.9+）"
        @click="openSprite"
      >图标</button>
      <button
        v-if="supportsObject"
        :class="{ pulse: toolbarPulse === '头像' }"
        type="button"
        title="插入内嵌玩家头像（1.21.9+）"
        @click="openPlayer"
      >头像</button>
      <button :class="{ pulse: toolbarPulse === '交互' }" type="button" title="给选中文字添加悬停/点击/插入事件" @click="openEvents">交互</button>
      <button :class="{ pulse: toolbarPulse === '组件' }" type="button" title="插入翻译/按键/选择器/计分板/NBT 组件" @click="openComponent">组件</button>
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
            <label>命名颜色（点击直接套用）</label>
            <div class="named-row">
              <button
                v-for="c in namedColors"
                :key="c.name"
                :style="{ background: c.hex }"
                :title="c.name"
                type="button"
                @click="applyNamedColor(c.name, c.hex)"
              ></button>
            </div>
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

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="fontModalOpen" class="modal-overlay" @mousedown.self="fontModalOpen = false">
        <div class="modal-card color-card">
          <h2>字体</h2>
          <div class="color-page">
            <label>选择字体</label>
            <CustomSelect v-model="fontChoice" :options="fontOptions" />
            <template v-if="fontChoice === '__custom__'">
              <label>自定义字体 ID</label>
              <input v-model="customFont" type="text" placeholder="例如 minecraft:uniform" />
            </template>
          </div>
          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="fontModalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applyFont">应用</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="spriteModalOpen" class="modal-overlay" @mousedown.self="spriteModalOpen = false">
        <div class="modal-card color-card">
          <h2>插入图标</h2>
          <div class="color-page">
            <label>贴图来源</label>
            <CustomSelect v-model="spriteKind" :options="spriteKindOptions" />
            <label>选择方块 / 物品（自动填贴图路径）</label>
            <CatalogCombo v-model="spritePick" :catalog="spriteKind === 'block' ? BLOCKS : ITEMS" placeholder="搜索方块/物品" />
            <label>贴图路径 sprite</label>
            <input v-model="spritePath" type="text" placeholder="例如 block/stone" />
            <label>图集 atlas</label>
            <input v-model="spriteAtlas" type="text" placeholder="minecraft:blocks" />
          </div>
          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="spriteModalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applySprite">插入</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="playerModalOpen" class="modal-overlay" @mousedown.self="playerModalOpen = false">
        <div class="modal-card color-card">
          <h2>插入头像</h2>
          <div class="color-page">
            <label>玩家名 / UUID</label>
            <input v-model="playerName" type="text" placeholder="例如 Notch" />
            <label class="check-line"><input v-model="playerHat" type="checkbox" />显示帽子层</label>
          </div>
          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="playerModalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applyPlayer">插入</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="eventModalOpen" class="modal-overlay" @mousedown.self="eventModalOpen = false">
        <div class="modal-card color-card">
          <h2>交互事件</h2>
          <div class="color-page">
            <label>插入文本 insertion（Shift+点击填入聊天栏）</label>
            <input v-model="insertionText" type="text" placeholder="留空则不设置" />

            <label>点击事件 click_event</label>
            <CustomSelect v-model="clickAction" :options="clickActionOptions" />
            <template v-if="clickAction">
              <label>{{ clickValueLabel }}</label>
              <input v-model="clickValue" type="text" />
            </template>

            <label>悬停事件 hover_event</label>
            <CustomSelect v-model="hoverAction" :options="hoverActionOptions" />
            <template v-if="hoverAction === 'show_text'">
              <label>悬停文本</label>
              <input v-model="hoverText" type="text" />
            </template>
            <template v-else-if="hoverAction === 'show_item'">
              <label>物品 ID</label>
              <input v-model="hoverItemId" type="text" placeholder="minecraft:stone" />
              <label>数量</label>
              <NumberInput v-model="hoverItemCount" :min="1" />
            </template>
            <template v-else-if="hoverAction === 'show_entity'">
              <label>实体类型</label>
              <input v-model="hoverEntityType" type="text" placeholder="minecraft:pig" />
              <label>UUID</label>
              <input v-model="hoverEntityUuid" type="text" placeholder="可留空" />
              <label>名称</label>
              <input v-model="hoverEntityName" type="text" placeholder="可留空" />
            </template>
          </div>
          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="eventModalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applyEvents">应用</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>

  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="compModalOpen" class="modal-overlay" @mousedown.self="compModalOpen = false">
        <div class="modal-card color-card">
          <h2>插入组件</h2>
          <div class="color-page">
            <label>类型</label>
            <CustomSelect v-model="compType" :options="compTypeOptions" />

            <template v-if="compType === 'translatable'">
              <label>翻译键 translate</label>
              <input v-model="transKey" type="text" placeholder="例如 item.minecraft.diamond" />
              <label>缺省文本 fallback（可选）</label>
              <input v-model="transFallback" type="text" placeholder="翻译缺失时显示" />
              <label>参数 with（多个用 | 分隔，可选）</label>
              <input v-model="transWith" type="text" placeholder="参数1|参数2" />
            </template>

            <template v-else-if="compType === 'keybind'">
              <label>按键 keybind</label>
              <input v-model="keybindKey" type="text" placeholder="key.jump" />
            </template>

            <template v-else-if="compType === 'selector'">
              <label>选择器 selector</label>
              <input v-model="selectorValue" type="text" placeholder="@p" />
              <label>分隔符 separator（可选）</label>
              <input v-model="selectorSep" type="text" placeholder="例如 , " />
            </template>

            <template v-else-if="compType === 'score'">
              <label>目标 name</label>
              <input v-model="scoreName" type="text" placeholder="@s / 玩家名" />
              <label>计分项 objective</label>
              <input v-model="scoreObjective" type="text" placeholder="例如 kills" />
            </template>

            <template v-else>
              <label>NBT 路径</label>
              <input v-model="nbtPath" type="text" placeholder="例如 Health" />
              <label>来源 source</label>
              <CustomSelect v-model="nbtSource" :options="nbtSourceOptions" />
              <label>{{ nbtTargetLabel }}</label>
              <input v-model="nbtTarget" type="text" />
              <label class="check-line"><input v-model="nbtInterpret" type="checkbox" />解析为 JSON 文本（interpret）</label>
            </template>
          </div>
          <div class="modal-actions">
            <button class="normal-btn" type="button" @click="compModalOpen = false">取消</button>
            <button class="primary-btn" type="button" @click="applyComponent">插入</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
