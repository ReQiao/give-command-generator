<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import CatalogCombo from "./components/CatalogCombo.vue";
import CustomSelect from "./components/CustomSelect.vue";
import EffectEditor from "./components/EffectEditor.vue";
import InfoTip from "./components/InfoTip.vue";
import ItemPickerModal from "./components/ItemPickerModal.vue";
import AiPanel from "./components/AiPanel.vue";
import DeployPanel from "./components/DeployPanel.vue";
import NumberInput from "./components/NumberInput.vue";
import RichTextEditor from "./components/RichTextEditor.vue";
import AuthModal from "./components/AuthModal.vue";
import { installLiquidGlass } from "./logic/glass";
import { playIntro } from "./logic/intro";
// 背景里那盏灯。放 src/assets 而不是 public：走 Vite 的资源管线会带内容哈希，
// 换图之后不会因为浏览器缓存显示旧的。
import lanternUrl from "./assets/soul-lantern.png";
import {
  authModalMode,
  authModalOpen,
  gated as authGated,
  openAuth,
  pendingAiSwitch,
  recheckAuth,
  refreshAuth,
} from "./logic/auth";
import {
  ATTRIBUTES,
  BEDROCK_BLOCKS,
  BEDROCK_ITEMS,
  BLOCKS,
  CORRECT_FOR_DROPS,
  ENCHANTS,
  ITEM_LOCK_MODES,
  ITEMS,
  LIMIT_TYPES,
  OPERATIONS,
  RARITIES,
  SLOTS,
  VERSIONS,
} from "./data/catalog";
import {
  buildGiveCommand,
  createDefaultForm,
  fmtNumber,
  mapCatalog,
  matches,
  isJava121LegacyFamily,
  isJava1205Family,
  isJava1212Family,
  getModernProfile,
  normalizeForm,
  pairText,
  type AttributeRow,
  type BlockLimitRow,
  type EnchantRow,
  type GiveForm,
  type ToolRuleRow,
} from "./logic/builder";
import "./style.css";

interface SelectOption {
  label: string;
  value: string;
  description?: string;
}

const autosaveKey = "give-generator-pyside-autosave";
const animationKey = "give-generator-animation";

// ---------------- 使用须知（EULA）：必须同意才能进入软件 ----------------
// 版本号写进 key 里——以后条款有实质性修改，把这个数字改大，老用户会被
// 重新要求同意一遍，而不是永远沿用当初点过的那次同意。
// v2：加了账号体系之后要收手机号，按《个人信息保护法》必须单独告知并取得同意。
// 只改文案不改这个数字的话，已经点过同意的老用户永远看不到新条款，那份告知等于没做。
const EULA_VERSION = "2";
const eulaKey = `give-generator-eula-accepted-v${EULA_VERSION}`;
const eulaAccepted = ref(localStorage.getItem(eulaKey) === "true");
const eulaScrolledToEnd = ref(false);
const eulaTextEl = ref<HTMLElement | null>(null);

function checkEulaScrolled() {
  const el = eulaTextEl.value;
  if (!el) return;
  // 容差 8px：字体渲染/滚动条误差，卡在最后几像素不该拦着用户点不了同意。
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 8) eulaScrolledToEnd.value = true;
}

function acceptEula() {
  eulaAccepted.value = true;
  localStorage.setItem(eulaKey, "true");
  // 第一次用的人是在这一刻才第一次看到主界面的，开场动画该放给他看——
  // 否则"先只展示背景再落位"这段只有老用户（已经同意过的）才见得到。
  void nextTick(() => {
    if (shellEl.value) void playIntro(shellEl.value).finally(() => (introPending.value = false));
  });
}

function declineEula() {
  // 桌面端（Tauri webview）关掉这个窗口等于退出程序；浏览器里 close() 可能被拦，
  // 拦不掉也没关系——不同意就是不让往下走，界面本来就还停在须知页。
  window.close();
}
const builtinTemplateModules = import.meta.glob("../templates/*.json", { eager: true, import: "default" }) as Record<string, unknown>;

const status = ref("状态：未生成");
const form = reactive<GiveForm>(loadAutosave());
const preview = ref("");
const activeTab = ref(form.version === "bedrock" ? "基岩选项" : "文本");
const foodToolTab = ref("食物消耗");
const animationEnabled = ref(loadAnimation());
const dirty = ref(false);
const toastText = ref("");
const modal = reactive({ open: false, title: "", message: "", error: false });
const fileInput = ref<HTMLInputElement | null>(null);
const selectedBuiltinTemplate = ref("");
const itemPickerOpen = ref(false);
const pickBtnEl = ref<HTMLButtonElement | null>(null);
/** 手动填表 / AI 自然语言，两种模式共用顶部的版本选择。 */
const mode = ref<"manual" | "ai">("manual");

/**
 * 切模式。未登录点「AI 模式」时**不切过去**——停在手动模式，直接把登录框弹出来。
 *
 * 为什么不是像以前那样"先切进 AI 面板、再在面板里显示一块登录提示"：那样用户
 * 已经离开了手动模式，界面整个换了一套，却什么都干不了；而且切过去的那一下会
 * 放点灯动画，等于为一个进不去的地方演了一遍开场。现在动画只在真正能用的时候放。
 *
 * 登录成功后由 onAuthed 补上这次切换，那时 AiPanel 的 active 从 false 变 true，
 * 点灯动画照常触发——用户看到的顺序是「登录 → 灯亮 → 进 AI」，比反过来顺。
 */
function selectMode(next: "manual" | "ai") {
  if (next === "ai" && authGated.value) {
    pendingAiSwitch.value = true;
    openAuth("login");
    // 顺手再确认一次门禁开关：如果是"启动那一刻连不上服务器"导致的误判，
    // 这一次刷新就能纠正过来，用户不用重启软件。
    void recheckAuth().then(() => {
      if (!authGated.value && pendingAiSwitch.value) {
        pendingAiSwitch.value = false;
        authModalOpen.value = false;
        mode.value = "ai";
      }
    });
    return;
  }
  mode.value = next;
}

/**
 * 用户没登录就把弹窗关掉了，那次"我要进 AI 模式"的意图就此作废。
 * 不清掉的话它会一直挂着，等到很久以后用户从账号区点登录时突然跳进 AI 模式——
 * 那是他这次完全没要求过的事。
 */
watch(authModalOpen, (open) => {
  if (!open) pendingAiSwitch.value = false;
});

/** 登录/注册成功。只有"被 AI 门禁拦下来"那条路径才顺势切进 AI 模式。 */
async function onAuthed() {
  // 【必须同步取】AuthModal 的顺序是 emit("authed") → open = false，而下面要
  // await。等 await 回来时，上面那个"关窗就清空 pendingAiSwitch"的 watch 早就
  // 跑过了，读到的一定是 false，模式切换永远不会发生。
  const wanted = pendingAiSwitch.value;
  pendingAiSwitch.value = false;
  await refreshAuth();
  if (wanted && !authGated.value) mode.value = "ai";
}
const generateButtonText = ref("生成指令");
const copyButtonText = ref("复制指令");
const rowFlash = reactive<Record<string, boolean>>({});
let toastTimer: number | undefined;

const enchantText = ref("耐久");
const enchantLevel = ref(1);
const selectedEnchantRow = ref(-1);
const attrText = ref("攻击伤害");
const attrAmount = ref(1);
const attrSlot = ref("任意");
const attrOperation = ref("加算");
const attrId = ref("");
const selectedAttrRow = ref(-1);
const blockSearch = ref("");
const blockText = ref("石头");
const blockType = ref("可放置");
const selectedBlockRow = ref(-1);
const toolBlock = ref("石头");
const toolRuleSpeed = ref(1);
const toolCorrect = ref("默认");
const selectedToolRow = ref(-1);

const visibleTabs = computed(() => {
  if (form.version === "bedrock") return ["方块", "基岩选项"];
  if (isJava121LegacyFamily(form.version)) return ["文本", "附魔", "属性", "方块", "基础", "食物工具"];
  return ["文本", "附魔", "属性", "方块", "基础", "死亡效果", "食物工具"];
});

/**
 * 物品/方块下拉用的目录要跟着版本走：基岩版是另一套 ID 体系，条目也不完全重合
 * （基岩独有 62 个物品 / 157 个方块，Java 独有的那些在基岩里根本不存在）。
 * 选基岩版时就该只看得到基岩真实有的东西，否则用户能选到一个基岩里没有的物品，
 * 生成出来的指令进游戏直接报错。
 *
 * 注意"食物工具"页的 toolBlock 不走这里——tool 组件是 Java 1.20.5+ 独有的，
 * 基岩版的 give 根本不输出它（见 buildBedrock），那边继续用 Java 方块表是对的。
 */
const bedrockMode = computed(() => form.version === "bedrock");
const itemCatalog = computed(() => (bedrockMode.value ? BEDROCK_ITEMS : ITEMS));
const blockCatalog = computed(() => (bedrockMode.value ? BEDROCK_BLOCKS : BLOCKS));
const filteredBlocks = computed(() =>
  blockCatalog.value.filter((row) => matches(row, blockSearch.value)),
);
/**
 * 动画总开关挂到 <html> 上，不挂在 .app-shell 上。
 *
 * 原因见 style.css 里 .no-motion 那段：弹窗/下拉/toast 都 Teleport 到 body，
 * 挂在 shell 上的话它们全在选择器覆盖范围之外，关了动画照动。
 */
watch(
  animationEnabled,
  (on) => document.documentElement.classList.toggle("no-motion", !on),
  { immediate: true },
);

const shellClass = computed(() => ({ "app-shell": true }));
const shellEl = ref<HTMLElement | null>(null);

/**
 * 开场动画还没开始，界面要藏着。
 *
 * 【必须在模板里就是 opacity:0，不能等 playIntro 去设】playIntro 跑在挂载之后，
 * 那时第一帧已经画出去了——用户会先看到一整块完整的界面，然后它消失、再淡入。
 * 实测过这个 bug：外层玻璃的不透明度是 1.00 → 0.49 → 1.00，正好反了。
 *
 * 关了界面动画时初值就是 false，界面直接显示，不经过任何隐藏状态。
 */
const introPending = ref(
  animationEnabled.value &&
    !(typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches),
);
const legacyJava = computed(() => isJava121LegacyFamily(form.version));
const supportsTooltipDisplay = computed(() =>
  !isJava121LegacyFamily(form.version) &&
  !isJava1205Family(form.version) &&
  !isJava1212Family(form.version)
);
const supportsGlider = computed(() => getModernProfile(form.version).supportsGlider);
const supportsDeathProtection = computed(() => getModernProfile(form.version).supportsDeathProtection);
const versionOptions = computed(() => pairOptions(VERSIONS, "value"));
const slotOptions = computed(() => pairOptions(SLOTS));
const operationOptions = computed(() => pairOptions(OPERATIONS));
const limitTypeOptions = computed(() => pairOptions(LIMIT_TYPES));
const rarityOptions = computed(() => pairOptions(RARITIES));
const itemLockOptions = computed(() => pairOptions(ITEM_LOCK_MODES));
const correctForDropsOptions = computed(() => pairOptions(CORRECT_FOR_DROPS));
const glintOptions = computed(() => textOptions(["默认", "开启", "关闭"]));
const yesNoDefaultOptions = computed(() => textOptions(["默认", "是", "否"]));
const builtinTemplates = computed(() =>
  Object.entries(builtinTemplateModules).map(([path, data]) => {
    const fallback = path.split(/[\\/]/).pop()?.replace(/\.json$/i, "") || "未命名模板";
    const normalized = normalizeForm(data);
    return {
      label: normalized.templateName || fallback,
      value: fallback,
      data: normalized,
    };
  }),
);
const builtinTemplateOptions = computed<SelectOption[]>(() => [
  { label: "选择内置模板", value: "" },
  ...builtinTemplates.value.map((template) => ({
    label: template.label,
    value: template.value,
    description: "内置 JSON 模板",
  })),
]);
const targetCatalog = [
  ["@s", "@s", "自己 self"],
  ["@p", "@p", "最近玩家 nearest player"],
  ["@a", "@a", "全部玩家 all players"],
  ["@r", "@r", "随机玩家 random player"],
  ["@e", "@e", "全部实体 all entities"],
] as const;

watch(
  form,
  () => {
    dirty.value = true;
    refreshPreviewIfGenerated();
  },
  { deep: true },
);

watch(animationEnabled, (value) => {
  localStorage.setItem(animationKey, JSON.stringify(value));
  showToast(value ? "界面动画已开启" : "界面动画已关闭");
});

watch(
  () => form.version,
  () => {
    pruneUnsupportedOptionsForVersion();
    if (!visibleTabs.value.includes(activeTab.value)) {
      activeTab.value = form.version === "bedrock" ? "基岩选项" : "文本";
    }
    if (!["食物消耗", "食用效果", "工具规则"].includes(foodToolTab.value)) {
      foodToolTab.value = "食物消耗";
    }
    status.value = form.version === "bedrock" ? "状态：基岩版模式" : `状态：${pairText(VERSIONS, form.version)} 模式`;
    refreshPreviewIfGenerated();
  },
);

const autosaveTimer = window.setInterval(() => {
  if (!dirty.value) return;
  localStorage.setItem(autosaveKey, JSON.stringify(form));
  dirty.value = false;
  status.value = "状态：已自动保存";
}, 1000);

onBeforeUnmount(() => {
  window.clearInterval(autosaveTimer);
});

onMounted(() => {
  // 须知内容如果短到不用滚动就能看完（小字号/大窗口），不该因为用户压根没机会
  // 触发 scroll 事件就一直卡在"未同意"按钮不能点，挂载后主动检查一次。
  void nextTick(checkEulaScrolled);
  // 登录态在这一层拉，不放 AiPanel 里：门禁判断发生在这儿的模式切换按钮上，
  // 拉取要早于用户可能点到「AI 模式」的那一刻。
  void recheckAuth();
  // 液态玻璃：按选择器认领所有浮层，后来 v-if 挂上来的弹窗也会自动接管。
  // 不支持 SVG 滤镜的引擎（macOS 的 WKWebView）里它直接空转，交给 CSS 降级。
  // 灯的图片路径是 Vite 打过哈希的，CSS 写不出来，运行时注入给 .shell-frost 用。
  document.documentElement.style.setProperty("--bg-lantern", `url(${lanternUrl})`);
  uninstallGlass = installLiquidGlass();
  // 开场：先只有背景，界面再一块块落位。节奏和"为什么用 WAAPI"见 logic/intro.ts。
  // 须知门禁还没过的时候不放——那时 shell 根本没挂载，等用户点完同意再说。
  void nextTick(() => {
    if (shellEl.value) void playIntro(shellEl.value).finally(() => (introPending.value = false));
  });
});

let uninstallGlass: (() => void) | undefined;
onBeforeUnmount(() => uninstallGlass?.());

function loadAutosave(): GiveForm {
  const saved = localStorage.getItem(autosaveKey);
  if (!saved) return createDefaultForm();
  try {
    const form = normalizeForm(JSON.parse(saved));
    status.value = "状态：已恢复上次内容";
    return form;
  } catch {
    return createDefaultForm();
  }
}

function loadAnimation(): boolean {
  const saved = localStorage.getItem(animationKey);
  return saved === null ? true : saved === "true";
}

function selectItem(name: string) {
  form.item = name;
  pulseRow("item");
}

function addEnchant() {
  const row: EnchantRow = { id: enchantText.value, level: enchantLevel.value };
  form.enchantments.push(row);
  selectedEnchantRow.value = form.enchantments.length - 1;
  pulseRow(`enchant-${selectedEnchantRow.value}`);
}

function addAttribute() {
  const row: AttributeRow = {
    type: attrText.value,
    amount: attrAmount.value,
    slot: attrSlot.value,
    operation: attrOperation.value,
    id: attrId.value || String(Date.now() + form.attributes.length),
  };
  form.attributes.push(row);
  selectedAttrRow.value = form.attributes.length - 1;
  pulseRow(`attr-${selectedAttrRow.value}`);
}

function addBlock() {
  const row: BlockLimitRow = { block: blockText.value, type: blockType.value };
  form.blockLimits.push(row);
  selectedBlockRow.value = form.blockLimits.length - 1;
  pulseRow(`block-${selectedBlockRow.value}`);
}

function addToolRule() {
  const row: ToolRuleRow = {
    blocks: [mapCatalog(BLOCKS, toolBlock.value)],
    speed: fmtNumber(toolRuleSpeed.value),
    correct_for_drops: toolCorrect.value,
  };
  form.toolRules.push(row);
  selectedToolRow.value = form.toolRules.length - 1;
  pulseRow(`tool-${selectedToolRow.value}`);
}

function removeSelected<T>(rows: T[], selected: { value: number }) {
  if (selected.value < 0) return;
  rows.splice(selected.value, 1);
  selected.value = Math.min(selected.value, rows.length - 1);
}

function removeEnchant() {
  removeSelected(form.enchantments, selectedEnchantRow);
}

function removeAttribute() {
  removeSelected(form.attributes, selectedAttrRow);
}

function removeBlock() {
  removeSelected(form.blockLimits, selectedBlockRow);
}

function removeToolRule() {
  removeSelected(form.toolRules, selectedToolRow);
}

function generate() {
  try {
    const command = buildGiveCommand(form);
    preview.value = command;
    status.value = `状态：已生成，长度 ${command.length}`;
    pulseButton(generateButtonText, "已生成", "生成指令");
    pulseRow("preview");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    status.value = `错误：${message}`;
    showMessage("生成失败", message, true);
  }
}

function refreshPreviewIfGenerated() {
  if (!preview.value.trim()) return;
  try {
    preview.value = buildGiveCommand(form);
  } catch {
    // Keep the last valid generated command while the user is editing an incomplete form.
  }
}

function pruneUnsupportedOptionsForVersion() {
  if (isJava121LegacyFamily(form.version)) {
    form.glider = false;
    form.deathProtection = false;
    form.deathEffects = [];
    form.hiddenComponents = "";
    form.consumeSound = "";
    form.consumeParticles = "默认";
    if (activeTab.value === "死亡效果") activeTab.value = "基础";
  } else if (isJava1205Family(form.version)) {
    form.glider = false;
    form.deathProtection = false;
    form.deathEffects = [];
    form.hiddenComponents = "";
    form.consumableEnabled = false;
    form.consumeSound = "";
    form.consumeParticles = "默认";
    form.consumeEffects = [];
    form.attributes = [];
    if (activeTab.value === "死亡效果") activeTab.value = "基础";
  } else if (isJava1212Family(form.version)) {
    form.hiddenComponents = "";
  }
}

function applyFormData(value: unknown) {
  Object.assign(form, normalizeForm(value));
  pruneUnsupportedOptionsForVersion();
  refreshPreviewIfGenerated();
}

async function copy() {
  try {
    await navigator.clipboard.writeText(preview.value);
    pulseButton(copyButtonText, "已复制", "复制指令");
    pulseRow("preview");
    showToast("已复制");
  } catch {
    showToast("复制失败，请手动复制");
  }
}

async function saveTemplate() {
  const payload = JSON.stringify(form, null, 2);
  const filename = `${(form.templateName.trim() || "未命名模板").replace(/[\\/:*?"<>|]/g, "_")}.json`;

  if (isTauri()) {
    try {
      const path = await save({
        defaultPath: filename,
        filters: [{ name: "JSON 模板", extensions: ["json"] }],
      });
      if (!path) return;
      await writeTextFile(path, payload);
      showToast(`模板已保存到 ${path}`, 4000);
    } catch (error) {
      showMessage("保存失败", error instanceof Error ? error.message : String(error), true);
    }
    return;
  }

  const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  link.click();
  URL.revokeObjectURL(link.href);
  showToast("模板已保存到浏览器默认下载目录");
}

function loadTemplate() {
  fileInput.value?.click();
}

async function handleTemplateFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;

  try {
    const text = await file.text();
    applyFormData(JSON.parse(text));
    status.value = "状态：模板已读取";
    showToast("模板已读取");
  } catch (error) {
    showMessage("读取失败", error instanceof Error ? error.message : String(error), true);
  }
}

function applyBuiltinTemplate(value: string) {
  selectedBuiltinTemplate.value = value;
  if (!value) return;
  const template = builtinTemplates.value.find((item) => item.value === value);
  if (!template) return;
  applyFormData(template.data);
  status.value = `状态：已载入内置模板 ${template.label}`;
  showToast("内置模板已载入");
}

function showMessage(title: string, message: string, error = false) {
  modal.title = title;
  modal.message = message;
  modal.error = error;
  modal.open = true;
}

function showToast(message: string, duration = 1800) {
  toastText.value = message;
  if (toastTimer !== undefined) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastText.value = "";
    toastTimer = undefined;
  }, duration);
}

function pulseButton(target: { value: string }, text: string, original: string) {
  target.value = text;
  window.setTimeout(() => {
    target.value = original;
  }, 700);
}

function pulseRow(key: string) {
  rowFlash[key] = false;
  window.requestAnimationFrame(() => {
    rowFlash[key] = true;
    window.setTimeout(() => {
      rowFlash[key] = false;
    }, 580);
  });
}

function displayBlocks(value: string[] | string): string {
  return Array.isArray(value) ? value.join(",") : value;
}

function pairOptions(rows: readonly (readonly [string, string, ...unknown[]])[], mode: "label" | "value" = "label"): SelectOption[] {
  return rows.map((row) => ({
    label: String(row[0]),
    value: mode === "value" ? String(row[1]) : String(row[0]),
    description: typeof row[2] === "string" ? row[2] : undefined,
  }));
}

function textOptions(items: string[]): SelectOption[] {
  return items.map((item) => ({ label: item, value: item }));
}
</script>

<template>
  <!-- 按钮用的折射滤镜。
       面板/弹窗**不在这里**——它们的折射由 logic/glass.ts 在运行时按元素实际
       尺寸生成（边缘位移图，中间不扭、只有边缘一圈弯折，见那个文件的头注释）。
       按钮留在这套 feTurbulence 里是权衡的结果：位移图必须逐元素按尺寸生成，
       而按钮又多又小，给每个按钮挂一个 ResizeObserver 加一份 4KB 的滤镜串
       不划算；而按钮那么小，边缘折射和噪声折射肉眼也分不出来。 -->
  <svg width="0" height="0" style="position: absolute" aria-hidden="true">
    <defs>
      <filter id="lensBtnNormal" x="-40%" y="-40%" width="180%" height="180%">
        <feTurbulence type="fractalNoise" baseFrequency="0.02 0.03" numOctaves="2" seed="3" result="noise" />
        <feGaussianBlur in="noise" stdDeviation="5" result="soft" />
        <feDisplacementMap in="SourceGraphic" in2="soft" scale="14" xChannelSelector="R" yChannelSelector="G" />
      </filter>
      <filter id="lensBtnPressed" x="-50%" y="-50%" width="200%" height="200%">
        <feTurbulence type="fractalNoise" baseFrequency="0.017 0.026" numOctaves="2" seed="27" result="noise" />
        <feGaussianBlur in="noise" stdDeviation="4" result="soft" />
        <feDisplacementMap in="SourceGraphic" in2="soft" scale="34" xChannelSelector="R" yChannelSelector="G" />
      </filter>
    </defs>
  </svg>

  <!--
    背景层。三件东西叠在一起，从下到上：
      1. body 那片三段蓝色渐变（在 style.css 里）
      2. 一圈以灯为中心的青色光晕，越靠中心越亮
      3. 灵魂灯笼本体

    【光晕是静止的，不做呼吸动画】两个原因：一是 backdrop-filter 的结果依赖
    背后的合成内容，背景只要一动，上面所有玻璃每帧都要重算，整个界面会掉到
    个位数帧率（这一课已经付过学费了）；二是 apple-design 那份材质指南里明确
    提到要避开"全视口缓慢循环的背景动效"，那正是最容易引发不适的一类。

    aria-hidden：纯装饰，读屏软件不该念它。
  -->
  <div class="app-bg" aria-hidden="true">
    <div class="app-bg-glow"></div>
    <img :src="lanternUrl" class="app-bg-lantern" alt="" draggable="false" />
  </div>

  <div v-if="!eulaAccepted" class="eula-gate">
    <div class="eula-box">
      <h2>使用须知与隐私说明</h2>
      <div ref="eulaTextEl" class="eula-text" @scroll="checkEulaScrolled">
        <p class="eula-lead">
          本软件分为<strong>手动模式</strong>和 <strong>AI 模式</strong>两部分。
          手动模式完全在你的电脑本地运行，<strong>不联网、不收集任何信息</strong>；
          只有当你主动使用 AI 模式时，才需要注册账号并联网。下面第三、四节专门讲这件事，请重点看。
        </p>

        <h3>一、版权与授权</h3>
        <p>
          本软件（含全部源代码、界面设计，以及"自然语言 → AI 意图 → 确定性指令构建器"这一实现方式）
          版权归开发者所有，受《中华人民共和国著作权法》及相关法律法规保护，未经明示授予的权利均予保留。
        </p>
        <p>
          开发者仅授予你在自有设备上运行本软件、使用其生成的游戏内指令的权利；
          你因安装或使用本软件，不因此获得对源代码本身的任何权利。
        </p>

        <h3>二、禁止行为</h3>
        <p>未经开发者书面许可，你不得从事以下行为：</p>
        <ul>
          <li>对本软件进行反编译、反汇编、逆向工程，或以其他方式还原其源代码；</li>
          <li>复制、传播、出售、二次分发本软件的源代码或其实质性部分（包括但不限于指令构建逻辑）；</li>
          <li>移除、隐藏或篡改本软件内的版权声明、作者信息或本协议；</li>
          <li>
            以脚本、自动化程序或其他非正常手段批量注册账号、批量索取短信验证码、
            绕过计费或干扰服务器正常运行。
          </li>
        </ul>

        <h3>三、账号与手机号（个人信息处理告知）</h3>
        <p>
          <strong>只有使用 AI 模式才需要注册账号。</strong>注册时我们会收集你的
          <strong>用户名、密码和手机号码</strong>，用途仅限于以下三项，不作他用：
        </p>
        <ul>
          <li><strong>手机号</strong>——通过短信验证码完成注册验证、找回密码，以及在账号异常时作为唯一的身份凭据；</li>
          <li><strong>用户名与密码</strong>——用于登录。密码以加盐哈希形式存储，开发者无法查看你的原始密码；</li>
          <li><strong>灵魂币余额与消费记录</strong>——用于计费，也让你换电脑、重装系统之后余额还在。</li>
        </ul>
        <p>
          <strong>关于短信</strong>：验证码短信经由阿里云发出，短信开头方括号里显示的是
          <strong>短信服务商的签名，不是"灵魂灯笼"</strong>。这不是诈骗短信，请以你在本软件内主动点击"获取验证码"
          为准；你没有操作过却收到验证码时，请不要提供给任何人。
        </p>
        <p>
          <strong>存储与共享</strong>：上述信息存储在开发者自行租用的服务器上，
          <strong>不会出售、不会共享给任何第三方</strong>，也不用于广告或用户画像。
          手机号仅在向短信服务商发送验证码时传递给该服务商，用完即止。
        </p>
        <p>
          <strong>你的权利</strong>：你可以随时查询、更正你的账号信息，或要求注销账号并删除全部相关数据。
          注销后余额与消费记录一并清除且不可恢复。目前的办理渠道是本项目的
          GitHub Issues（<span class="eula-mono">github.com/ReQiao/give-command-generator</span>）。
        </p>
        <p>
          <strong>未成年人</strong>：如果你未满 14 周岁，请在监护人陪同下阅读本协议，
          并在取得监护人同意后再注册使用 AI 模式。
        </p>

        <h3>四、AI 生成、灵魂币与计费</h3>
        <p>
          AI 模式会把你输入的<strong>需求描述</strong>发送到开发者的服务器，再由服务器转发给第三方大模型服务商处理。
          请不要在描述里填写真实姓名、住址、账号密码等与生成指令无关的个人信息。
        </p>
        <p>
          AI 生成按<strong>真实调用量</strong>折算扣除灵魂币，不是固定单价。
          当前处于免费测试阶段，充值不会真实扣款；未来若开放付费，会在充值页面明确标示，
          <strong>不会在你不知情的情况下扣费</strong>。
        </p>
        <p>
          AI 的输出可能出错。软件已经用确定性构建器兜住语法合法性，
          但<strong>不保证生成结果符合你的预期</strong>，请在使用前自行确认。
        </p>

        <h3>五、免责</h3>
        <p>
          本软件按"现状"提供，不对因使用本软件（含 AI 生成的指令、一键部署功能）
          导致的存档损坏、数据丢失或其他后果承担责任，<strong>请自行做好存档备份后再使用一键部署功能</strong>。
        </p>
        <p>
          服务器可能因维护、故障、欠费、第三方服务中断等原因暂时或永久不可用。
          开发者会尽力维持服务，但不对服务的持续可用性作出承诺。
        </p>

        <h3>六、违约与协议变更</h3>
        <p>
          违反上述条款的，开发者保留通过一切合法手段（包括但不限于中止账号、公开说明情况、提起诉讼）
          追究相应法律责任的权利。
        </p>
        <p>
          本协议如有实质性修改，软件会在下次启动时再次向你完整展示并请求同意，
          不会沿用你此前的同意。
        </p>
        <p>
          点击下方"我已阅读并同意"，即表示你已完整阅读、理解并同意接受本协议的全部条款，
          <strong>并同意开发者按第三节所述的目的和范围处理你的手机号等个人信息</strong>；
          如不同意，请勿使用本软件。
        </p>
      </div>
      <div class="eula-actions">
        <button type="button" @click="declineEula">不同意（退出）</button>
        <button type="button" class="primary-btn" :disabled="!eulaScrolledToEnd" @click="acceptEula">
          {{ eulaScrolledToEnd ? "我已阅读并同意" : "请先滑到底部" }}
        </button>
      </div>
    </div>
  </div>

  <main
    v-else
    ref="shellEl"
    :class="shellClass"
    :style="introPending ? { opacity: 0 } : undefined"
  >
    <!-- 预先模糊好的背景副本，就是这块面板的"毛玻璃"。
         用它而不是 backdrop-filter 的原因见 style.css 里 .shell-frost 的注释。 -->
    <div class="shell-frost" aria-hidden="true"></div>
    <section class="card top-card">
      <div class="brand-group">
        <div class="logo"></div>
        <h1>Give指令生成器</h1>
        <div class="mode-switch" role="tablist">
          <button
            type="button"
            role="tab"
            :aria-selected="mode === 'manual'"
            :class="{ active: mode === 'manual' }"
            @click="selectMode('manual')"
          >手动模式</button>
          <button
            type="button"
            role="tab"
            :aria-selected="mode === 'ai'"
            :class="{ active: mode === 'ai' }"
            @click="selectMode('ai')"
          >AI 模式</button>
        </div>
      </div>
      <!--
        两套顶部工具条一直同时挂载，用 grid 叠在同一格里（跟下面 split-layout/ai-card
        用的是同一招），只用 opacity+inert 切换可见/可交互——绝不能用 v-if/v-else。
        原因：manual 工具条按钮多，AI 工具条只有一个版本选择器，两者自然高度不同；
        如果互斥挂载，切换模式那一刻顶部栏高度会瞬间变化，这一行是 CSS Grid 的
        "auto" 行，一变高度就挤压/让出下面 1fr 内容行的空间，看起来像整页跳了一下——
        这个跳动只跟"顶部栏瞬间变了多高"有关，跟界面动画开关完全无关，所以之前
        单纯做过渡动画/钉 grid-row 都没能根治。两套工具条一直都在，取两者中较高的
        那个作为顶部栏的固定高度，模式切换时顶部栏高度压根不会变，也就没有可跳的了。
      -->
      <div class="top-form-stack">
        <div class="top-form ai-top-form" :class="{ 'stack-hidden': mode !== 'ai' }" :inert="mode !== 'ai'">
          <span class="field-label">版本<InfoTip text="AI 生成的指令会按这个版本的语法构建。" /></span>
          <CustomSelect v-model="form.version" :options="versionOptions" />
        </div>
        <div class="top-form" :class="{ 'stack-hidden': mode === 'ai' }" :inert="mode === 'ai'">
          <span class="field-label">模板名<InfoTip text="保存模板时使用这个名称作为 JSON 文件名。" /></span>
          <input v-model="form.templateName" class="template-input" />
          <CustomSelect
            class="builtin-template-select"
            :model-value="selectedBuiltinTemplate"
            :options="builtinTemplateOptions"
            @update:model-value="applyBuiltinTemplate"
          />
          <button type="button" @click="saveTemplate">保存模板</button>
          <button type="button" @click="loadTemplate">读取模板</button>
          <button type="button" @click="copy">{{ copyButtonText }}</button>
          <button class="primary-btn" type="button" @click="generate">{{ generateButtonText }}</button>
          <input ref="fileInput" accept="application/json,.json" hidden type="file" @change="handleTemplateFile" />
        </div>
      </div>
    </section>

    <!-- v-show，不是 v-if：同上面手动面板的道理，切走再切回不能丢用户已经打好的字/生成结果。
         :active 单独传 mode==='ai'，给 AiPanel 用来判断"这次是不是刚切进来"，
         好在每次切入时重放点灯特效——面板本身常驻挂载，不能再靠组件创建时机触发动画了。 -->
    <AiPanel
      v-show="mode === 'ai'"
      :active="mode === 'ai'"
      :version="form.version"
      :animate="animationEnabled"
      @toast="showToast"
      @update:version="form.version = $event"
    />

    <!--
      故意不用 <Transition> 包裹手动内容：实测证明哪怕只给 enter 定义 CSS、
      leave 完全不定义过渡属性，Vue 的 <Transition> 组件本身（不是 CSS）也会
      用两次 requestAnimationFrame 做双缓冲来切换 class，这个 JS 层面的开销
      在低性能设备 / CPU 降速下会被放大成好几帧的延迟，足以让"旧内容还在
      但已经切到别的模式"这个冲突画面重新出现（4x CPU 降速下实测复现）。
      v-show 不套 Transition 就是纯粹的同步 display 切换，没有这个开销，
      这是唯一在降速测试下验证过绝对不会闪的写法。
    -->
    <section v-show="mode === 'manual'" class="split-layout">
      <aside class="card side-panel">
        <div class="form-grid">
          <span class="field-label">版本<InfoTip text="选择 Java 组件语法或基岩版基础 give 语法。Java 功能更多，基岩版更偏基础参数。" /></span>
          <CustomSelect v-model="form.version" :options="versionOptions" />

          <span class="field-label">目标<InfoTip text="@a 是所有玩家，@p 是最近玩家，@s 是自己，@e 是全部实体。" /></span>
          <CatalogCombo v-model="form.target" :catalog="targetCatalog" placeholder="@a" />

          <span class="field-label">物品<InfoTip text="可以输入中文名、minecraft:ID 或不带 minecraft: 的 ID，按 Tab 可补全；也可以点「选择」从分类列表里挑。" /></span>
          <div class="item-field">
            <CatalogCombo v-model="form.item" :catalog="itemCatalog" placeholder="选择或输入物品" />
            <button ref="pickBtnEl" class="pick-btn" type="button" @click="itemPickerOpen = true">选择…</button>
          </div>

          <span class="field-label">数量<InfoTip text="生成物品数量。可以直接输入，也可以用右侧箭头微调。" /></span>
          <NumberInput v-model="form.count" :min="1" />

          <span></span>
          <label class="check-line"><input v-model="form.withSlash" type="checkbox" />带斜杠</label>

          <span></span>
          <label class="check-line"><input v-model="animationEnabled" type="checkbox" />启用界面动画</label>
        </div>

        <p class="status-text">{{ status }}</p>
      </aside>

      <section class="card tab-panel">
        <div class="tab-strip">
          <button
            v-for="tab in visibleTabs"
            :key="tab"
            :class="{ active: activeTab === tab }"
            type="button"
            @click="activeTab = tab"
          >
            {{ tab }}
          </button>
        </div>

        <Transition name="tab-page" mode="out-in">
          <section :key="activeTab" class="tab-page">
            <div v-if="activeTab === '文本'" class="text-tab">
              <RichTextEditor v-model="form.displayName" :version="form.version" title="显示名称" @toast="showToast" />
              <RichTextEditor v-model="form.itemName" :version="form.version" title="物品名称" @toast="showToast" />
              <RichTextEditor v-model="form.lore" :version="form.version" multiline title="物品描述" @toast="showToast" />
            </div>

            <div v-else-if="activeTab === '附魔'" class="table-tab">
              <div class="inline-row">
                <span class="field-label">附魔<InfoTip text="输入中文、英文 ID 或缩写后按 Tab 补全；悬浮候选项可看到通俗说明。" /></span>
                <CatalogCombo v-model="enchantText" :catalog="ENCHANTS" explain placeholder="输入 耐 / unb / minecraft:unb" />
                <span class="field-label">等级<InfoTip text="附魔等级，允许高于原版常规上限，用于生成高等级物品。" /></span>
                <NumberInput v-model="enchantLevel" :min="1" />
                <button type="button" @click="addEnchant">添加</button>
                <button type="button" @click="removeEnchant">删除选中</button>
              </div>
              <table class="data-table">
                <thead><tr><th>附魔</th><th>等级</th></tr></thead>
                <tbody>
                  <tr
                    v-for="(row, index) in form.enchantments"
                    :key="index"
                    :class="{ selected: selectedEnchantRow === index, flash: rowFlash[`enchant-${index}`] }"
                    @click="selectedEnchantRow = index"
                  >
                    <td>{{ row.id }}</td>
                    <td>{{ row.level }}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div v-else-if="activeTab === '属性'" class="table-tab">
              <div class="inline-row attr-row">
                <span class="field-label">属性<InfoTip text="属性修饰符会改变物品持有或装备时的能力；悬浮候选项可查看作用解释。" /></span>
                <CatalogCombo v-model="attrText" :catalog="ATTRIBUTES" explain placeholder="属性名或 ID" />
                <span class="field-label">数值<InfoTip text="属性增减的数值。过大可能导致游戏内效果异常，请按用途调整。" /></span>
                <NumberInput v-model="attrAmount" :step="0.0001" />
                <span class="field-label">槽位<InfoTip text="限制属性在哪个装备槽或手持槽生效。任意表示不限制。" /></span>
                <CustomSelect v-model="attrSlot" :options="slotOptions" />
                <span class="field-label">运算<InfoTip text="加算是直接增减；基值乘算按基础值比例变化；总值乘算按最终值比例变化。" /></span>
                <CustomSelect v-model="attrOperation" :options="operationOptions" />
                <span class="field-label">ID<InfoTip text="属性修饰符的唯一标识。留空会自动生成，通常不需要手动填。" /></span>
                <input v-model="attrId" placeholder="留空自动生成" />
                <button type="button" @click="addAttribute">添加</button>
                <button type="button" @click="removeAttribute">删除选中</button>
              </div>
              <table class="data-table">
                <thead><tr><th>属性</th><th>数值</th><th>槽位</th><th>运算</th><th>ID</th></tr></thead>
                <tbody>
                  <tr
                    v-for="(row, index) in form.attributes"
                    :key="index"
                    :class="{ selected: selectedAttrRow === index, flash: rowFlash[`attr-${index}`] }"
                    @click="selectedAttrRow = index"
                  >
                    <td>{{ row.type }}</td>
                    <td>{{ row.amount }}</td>
                    <td>{{ row.slot }}</td>
                    <td>{{ row.operation }}</td>
                    <td>{{ row.id }}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div v-else-if="activeTab === '方块'" class="table-tab">
              <div class="inline-row">
                <span class="field-label">方块搜索<InfoTip text="过滤可放置 / 可破坏方块列表，可输入中文或英文 ID。" /></span>
                <input v-model="blockSearch" placeholder="搜索方块或输入英文ID" />
                <span class="field-label">方块<InfoTip text="用于 can_place_on 或 can_break / can_destroy 规则的方块 ID。" /></span>
                <CatalogCombo v-model="blockText" :catalog="filteredBlocks" placeholder="方块名或 ID" />
                <span class="field-label">类型<InfoTip text="可放置表示只能放到这些方块上；可破坏表示冒险模式可破坏这些方块；两者会同时生成。" /></span>
                <CustomSelect v-model="blockType" :options="limitTypeOptions" />
                <button type="button" @click="addBlock">添加</button>
                <button type="button" @click="removeBlock">删除选中</button>
              </div>
              <table class="data-table">
                <thead><tr><th>方块</th><th>类型</th></tr></thead>
                <tbody>
                  <tr
                    v-for="(row, index) in form.blockLimits"
                    :key="index"
                    :class="{ selected: selectedBlockRow === index, flash: rowFlash[`block-${index}`] }"
                    @click="selectedBlockRow = index"
                  >
                    <td>{{ row.block }}</td>
                    <td>{{ row.type }}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div v-else-if="activeTab === '基础'" class="basic-grid">
              <span class="field-label">稀有度<InfoTip text="影响物品名显示颜色和稀有度标记，不改变物品本身强度。" /></span>
              <CustomSelect v-model="form.rarity" :options="rarityOptions" />
              <span class="field-label">附魔光效<InfoTip text="强制开启或关闭附魔闪光；默认表示交给游戏按组件判断。" /></span>
              <CustomSelect v-model="form.glint" :options="glintOptions" />
              <span></span><label class="check-line"><input v-model="form.unbreakable" type="checkbox" />无法损坏<InfoTip text="物品不会因为使用而损失耐久。" /></label>
              <span v-if="supportsGlider"></span><label v-if="supportsGlider" class="check-line"><input v-model="form.glider" type="checkbox" />鞘翅飞行<InfoTip text="让物品拥有类似鞘翅的滑翔组件。" /></label>
              <span v-if="supportsDeathProtection"></span><label v-if="supportsDeathProtection" class="check-line"><input v-model="form.deathProtection" type="checkbox" />死亡保护<InfoTip text="死亡时触发保护组件，可配合死亡效果使用。" /></label>
              <span></span><label class="check-line"><input v-model="form.damageEnabled" type="checkbox" />启用当前损耗<InfoTip text="写入当前已经损失的耐久值。" /></label>
              <span class="field-label">当前损耗<InfoTip text="数值越大，物品越接近损坏。" /></span><NumberInput v-model="form.damage" :min="0" />
              <span></span><label class="check-line"><input v-model="form.maxDamageEnabled" type="checkbox" />启用最大耐久<InfoTip text="自定义物品最大耐久。" /></label>
              <span class="field-label">最大耐久<InfoTip text="物品可承受的总耐久值。" /></span><NumberInput v-model="form.maxDamage" :min="1" />
              <span></span><label class="check-line"><input v-model="form.stackEnabled" type="checkbox" />启用最大堆叠<InfoTip text="自定义物品最大堆叠数量。" /></label>
              <span class="field-label">最大堆叠<InfoTip text="通常不超过 99，实际表现以游戏版本为准。" /></span><NumberInput v-model="form.maxStackSize" :max="99" :min="1" />
              <span></span><label class="check-line"><input v-model="form.repairEnabled" type="checkbox" />启用修复消耗<InfoTip text="影响铁砧修复时的经验消耗。" /></label>
              <span class="field-label">修复消耗<InfoTip text="铁砧相关经验消耗数值。" /></span><NumberInput v-model="form.repairCost" :min="0" />
              <span v-if="supportsTooltipDisplay" class="field-label">隐藏组件<InfoTip text="用逗号分隔要隐藏在物品提示里的组件 ID。" /></span><input v-if="supportsTooltipDisplay" v-model="form.hiddenComponents" />
            </div>

            <EffectEditor
              v-else-if="activeTab === '死亡效果'"
              v-model="form.deathEffects"
              title="死亡效果"
              @toast="showToast"
            />

            <div v-else-if="activeTab === '食物工具'" class="food-tool">
              <div class="tab-strip compact">
                <button :class="{ active: foodToolTab === '食物消耗' }" type="button" @click="foodToolTab = '食物消耗'">食物消耗</button>
                <button :class="{ active: foodToolTab === '食用效果' }" type="button" @click="foodToolTab = '食用效果'">食用效果</button>
                <button :class="{ active: foodToolTab === '工具规则' }" type="button" @click="foodToolTab = '工具规则'">工具规则</button>
              </div>

              <div v-if="foodToolTab === '食物消耗'" class="basic-grid">
                <span></span><label class="check-line"><input v-model="form.foodEnabled" type="checkbox" />启用食物<InfoTip text="让物品拥有食物组件，可恢复饥饿值和饱和度。" /></label>
                <span class="field-label">营养值<InfoTip text="恢复多少饥饿值。" /></span><NumberInput v-model="form.nutrition" :min="0" />
                <span class="field-label">饱和度<InfoTip text="影响饥饿值消耗速度，越高越耐饿。" /></span><NumberInput v-model="form.saturation" :min="0" :step="0.001" />
                <span class="field-label">总是可食用<InfoTip text="是否允许满饥饿时也能食用。" /></span><CustomSelect v-model="form.alwaysEat" :options="yesNoDefaultOptions" />
                <span></span><label class="check-line"><input v-model="form.consumableEnabled" type="checkbox" />启用消耗<InfoTip text="添加使用 / 食用耗时、声音和粒子等消耗组件。" /></label>
                <span class="field-label">消耗时间<InfoTip text="使用或食用完成所需秒数。" /></span><NumberInput v-model="form.consumeSeconds" :min="0" :step="0.001" />
                <span v-if="!legacyJava" class="field-label">消耗声音<InfoTip text="消耗完成后播放的声音 ID，例如 minecraft:item.honey_bottle.drink。" /></span><input v-if="!legacyJava" v-model="form.consumeSound" />
                <span v-if="!legacyJava" class="field-label">消耗粒子<InfoTip text="控制食用或使用时是否显示粒子。" /></span><CustomSelect v-if="!legacyJava" v-model="form.consumeParticles" :options="yesNoDefaultOptions" />
              </div>

              <EffectEditor
                v-else-if="foodToolTab === '食用效果'"
                v-model="form.consumeEffects"
                title="食用效果"
                @toast="showToast"
              />

              <div v-else class="table-tab">
                <div class="basic-grid tool-form">
                  <span></span><label class="check-line"><input v-model="form.toolEnabled" type="checkbox" />启用工具<InfoTip text="让物品拥有工具组件，可控制挖掘速度和耐久损耗。" /></label>
                  <span class="field-label">默认挖掘速度<InfoTip text="未命中特定规则时的默认挖掘速度。" /></span><NumberInput v-model="form.defaultMiningSpeed" :min="0" :step="0.001" />
                  <span class="field-label">每方块损耗<InfoTip text="每破坏一个方块消耗多少耐久。" /></span><NumberInput v-model="form.damagePerBlock" :min="0" />
                </div>
                <div class="inline-row">
                  <span class="field-label">方块<InfoTip text="这条工具规则匹配的方块，可输入多个时用逗号分隔。" /></span>
                  <CatalogCombo v-model="toolBlock" :catalog="BLOCKS" placeholder="方块名或 ID" />
                  <span class="field-label">速度<InfoTip text="匹配这些方块时的挖掘速度。" /></span>
                  <NumberInput v-model="toolRuleSpeed" :min="0" :step="0.001" />
                  <span class="field-label">正确掉落<InfoTip text="控制该工具是否被视为能正确掉落该方块。" /></span>
                  <CustomSelect v-model="toolCorrect" :options="correctForDropsOptions" />
                  <button type="button" @click="addToolRule">添加规则</button>
                  <button type="button" @click="removeToolRule">删除选中</button>
                </div>
                <table class="data-table">
                  <thead><tr><th>方块</th><th>速度</th><th>正确掉落</th></tr></thead>
                  <tbody>
                    <tr
                      v-for="(row, index) in form.toolRules"
                      :key="index"
                      :class="{ selected: selectedToolRow === index, flash: rowFlash[`tool-${index}`] }"
                      @click="selectedToolRow = index"
                    >
                      <td>{{ displayBlocks(row.blocks) }}</td>
                      <td>{{ row.speed }}</td>
                      <td>{{ row.correct_for_drops }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>

            <div v-else-if="activeTab === '基岩选项'" class="basic-grid">
              <span class="field-label">数据值<InfoTip text="基岩版旧式 data value，多数物品填 0。" /></span><NumberInput v-model="form.bedrockDataValue" :max="32767" :min="0" />
              <span class="field-label">物品锁<InfoTip text="锁定背包或槽位，常用于地图和服务器道具。" /></span>
              <CustomSelect v-model="form.bedrockItemLock" :options="itemLockOptions" />
              <span></span><label class="check-line"><input v-model="form.bedrockKeepOnDeath" type="checkbox" />基岩死亡保留<InfoTip text="基岩版组件：死亡后保留该物品。" /></label>
              <p class="sub-text">基岩版当前生成：基础 give、数据值、可放置、可破坏、物品锁、死亡保留</p>
            </div>
          </section>
        </Transition>
      </section>
    </section>

    <section v-show="mode === 'manual'" class="card preview-card" :class="{ flash: rowFlash.preview }">
      <label>生成结果</label>
      <textarea id="preview" v-model="preview" placeholder="点击“生成指令”后，最终指令会显示在这里。" spellcheck="false"></textarea>
      <DeployPanel
        v-if="preview.trim() && form.version !== 'bedrock'"
        :commands="[preview]"
        :version="form.version"
        @toast="showToast"
        @update:version="form.version = $event"
      />
    </section>

    <Transition name="toast">
      <div v-if="toastText" class="toast">{{ toastText }}</div>
    </Transition>

    <ItemPickerModal
      v-model:open="itemPickerOpen"
      :catalog="itemCatalog"
      :current="form.item"
      :origin="pickBtnEl"
      :animate="animationEnabled"
      @select="selectItem"
    />

    <Transition name="modal-fade">
      <div v-if="modal.open" class="modal-overlay">
        <div :class="['modal-card', { shake: modal.error }]">
          <h2>{{ modal.title }}</h2>
          <p>{{ modal.message }}</p>
          <div class="modal-actions">
            <button class="primary-btn" type="button" @click="modal.open = false">知道了</button>
          </div>
        </div>
      </div>
    </Transition>
  </main>

  <!--
    登录弹窗提到 App 这一层来了。以前它挂在 AiPanel 里，但现在"点 AI 模式"
    这个动作本身就要弹它，而那个按钮在这儿；挂在 AiPanel 里就会出现
    「面板还没显示、弹窗却要弹」的尴尬。它自己 Teleport 到 body，
    所以放在 EULA 门禁的 v-else 分支外面也不影响层级。
  -->
  <AuthModal
    v-model:open="authModalOpen"
    :initial-mode="authModalMode"
    @authed="onAuthed"
    @toast="showToast"
  />
</template>
