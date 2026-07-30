<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import CatalogCombo from "./components/CatalogCombo.vue";
import CustomSelect from "./components/CustomSelect.vue";
import EffectEditor from "./components/EffectEditor.vue";
import InfoTip from "./components/InfoTip.vue";
import ItemPickerModal from "./components/ItemPickerModal.vue";
import NumberInput from "./components/NumberInput.vue";
import RichTextEditor from "./components/RichTextEditor.vue";
import {
  ATTRIBUTES,
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

const filteredBlocks = computed(() => BLOCKS.filter((row) => matches(row, blockSearch.value)));
const shellClass = computed(() => ({ "app-shell": true, "no-motion": !animationEnabled.value }));
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
  <main :class="shellClass">
    <section class="card top-card">
      <div class="brand-group">
        <div class="logo"></div>
        <h1>Give指令生成器</h1>
      </div>
      <div class="top-form">
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
        <button id="primary" type="button" @click="generate">{{ generateButtonText }}</button>
        <input ref="fileInput" accept="application/json,.json" hidden type="file" @change="handleTemplateFile" />
      </div>
    </section>

    <section class="split-layout">
      <aside class="card side-panel">
        <div class="form-grid">
          <span class="field-label">版本<InfoTip text="选择 Java 组件语法或基岩版基础 give 语法。Java 功能更多，基岩版更偏基础参数。" /></span>
          <CustomSelect v-model="form.version" :options="versionOptions" />

          <span class="field-label">目标<InfoTip text="@a 是所有玩家，@p 是最近玩家，@s 是自己，@e 是全部实体。" /></span>
          <CatalogCombo v-model="form.target" :catalog="targetCatalog" placeholder="@a" />

          <span class="field-label">物品<InfoTip text="可以输入中文名、minecraft:ID 或不带 minecraft: 的 ID，按 Tab 可补全；也可以点「选择」从分类列表里挑。" /></span>
          <div class="item-field">
            <CatalogCombo v-model="form.item" :catalog="ITEMS" placeholder="选择或输入物品" />
            <button class="pick-btn" type="button" @click="itemPickerOpen = true">选择…</button>
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

    <section class="card preview-card" :class="{ flash: rowFlash.preview }">
      <label>生成结果</label>
      <textarea id="preview" v-model="preview" placeholder="点击“生成指令”后，最终指令会显示在这里。" spellcheck="false"></textarea>
    </section>

    <Transition name="toast">
      <div v-if="toastText" class="toast">{{ toastText }}</div>
    </Transition>

    <ItemPickerModal
      v-model:open="itemPickerOpen"
      :catalog="ITEMS"
      :current="form.item"
      @select="selectItem"
    />

    <Transition name="modal-fade">
      <div v-if="modal.open" class="modal-overlay">
        <div :class="['modal-card', { shake: modal.error }]">
          <h2>{{ modal.title }}</h2>
          <p>{{ modal.message }}</p>
          <div class="modal-actions">
            <button id="primary" type="button" @click="modal.open = false">知道了</button>
          </div>
        </div>
      </div>
    </Transition>
  </main>
</template>
