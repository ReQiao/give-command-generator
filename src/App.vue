<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import EffectEditor from "./components/EffectEditor.vue";
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
  normalizeForm,
  type AttributeRow,
  type BlockLimitRow,
  type EnchantRow,
  type GiveForm,
  type ToolRuleRow,
} from "./logic/builder";
import "./style.css";

const autosaveKey = "give-generator-pyside-autosave";
const animationKey = "give-generator-animation";

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
  return ["文本", "附魔", "属性", "方块", "基础", "死亡效果", "食物工具"];
});

const filteredItems = computed(() => ITEMS.filter((row) => matches(row, form.itemSearch)));
const filteredBlocks = computed(() => BLOCKS.filter((row) => matches(row, blockSearch.value)));
const shellClass = computed(() => ({ "app-shell": true, "no-motion": !animationEnabled.value }));

watch(
  form,
  () => {
    dirty.value = true;
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
    if (!visibleTabs.value.includes(activeTab.value)) {
      activeTab.value = form.version === "bedrock" ? "基岩选项" : "文本";
    }
    status.value = form.version === "bedrock" ? "状态：基岩版模式" : "状态：Java 1.21.11+ 模式";
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

function saveTemplate() {
  const payload = JSON.stringify(form, null, 2);
  const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
  const link = document.createElement("a");
  const filename = `${(form.templateName.trim() || "未命名模板").replace(/[\\/:*?"<>|]/g, "_")}.json`;
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  link.click();
  URL.revokeObjectURL(link.href);
  showToast("模板已保存");
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
    Object.assign(form, normalizeForm(JSON.parse(text)));
    status.value = "状态：模板已读取";
    showToast("模板已读取");
  } catch (error) {
    showMessage("读取失败", error instanceof Error ? error.message : String(error), true);
  }
}

function showMessage(title: string, message: string, error = false) {
  modal.title = title;
  modal.message = message;
  modal.error = error;
  modal.open = true;
}

function showToast(message: string) {
  toastText.value = message;
  if (toastTimer !== undefined) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastText.value = "";
    toastTimer = undefined;
  }, 1800);
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
</script>

<template>
  <main :class="shellClass">
    <section class="card top-card">
      <div class="brand-group">
        <div class="logo">⛏</div>
        <h1>Give指令生成器</h1>
      </div>
      <div class="top-form">
        <label>模板名</label>
        <input v-model="form.templateName" class="template-input" />
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
          <label>版本</label>
          <select v-model="form.version">
            <option v-for="row in VERSIONS" :key="row[1]" :value="row[1]">{{ row[0] }}</option>
          </select>

          <label>目标</label>
          <input v-model="form.target" list="target-options" />
          <datalist id="target-options">
            <option value="@s"></option>
            <option value="@p"></option>
            <option value="@a"></option>
            <option value="@r"></option>
            <option value="@e"></option>
          </datalist>

          <label>物品搜索</label>
          <input v-model="form.itemSearch" placeholder="搜索物品或直接输入英文ID" />

          <label>物品</label>
          <input v-model="form.item" list="item-options" />
          <datalist id="item-options">
            <option v-for="row in filteredItems" :key="row[0]" :value="row[1]"></option>
          </datalist>

          <label>数量</label>
          <input v-model.number="form.count" min="1" type="number" />

          <span></span>
          <label class="check-line"><input v-model="form.withSlash" type="checkbox" />带斜杠</label>

          <span></span>
          <label class="check-line"><input v-model="animationEnabled" type="checkbox" />启用界面动画</label>
        </div>

        <div class="item-list" :class="{ flash: rowFlash.item }">
          <button
            v-for="row in filteredItems"
            :key="row[0]"
            :class="{ selected: form.item === row[1] }"
            type="button"
            @click="selectItem(row[1])"
          >
            {{ row[1] }}
          </button>
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
              <RichTextEditor v-model="form.displayName" title="显示名称" @toast="showToast" />
              <RichTextEditor v-model="form.itemName" title="物品名称" @toast="showToast" />
              <RichTextEditor v-model="form.lore" multiline title="物品描述" @toast="showToast" />
            </div>

            <div v-else-if="activeTab === '附魔'" class="table-tab">
              <div class="inline-row">
                <label>附魔</label>
                <input v-model="enchantText" list="enchant-options" />
                <datalist id="enchant-options">
                  <option v-for="row in ENCHANTS" :key="row[0]" :value="row[1]"></option>
                </datalist>
                <label>等级</label>
                <input v-model.number="enchantLevel" min="1" type="number" />
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
                <label>属性</label>
                <input v-model="attrText" list="attr-options" />
                <datalist id="attr-options">
                  <option v-for="row in ATTRIBUTES" :key="row[0]" :value="row[1]"></option>
                </datalist>
                <label>数值</label>
                <input v-model.number="attrAmount" step="0.0001" type="number" />
                <label>槽位</label>
                <select v-model="attrSlot"><option v-for="row in SLOTS" :key="row[1]">{{ row[0] }}</option></select>
                <label>运算</label>
                <select v-model="attrOperation"><option v-for="row in OPERATIONS" :key="row[1]">{{ row[0] }}</option></select>
                <label>ID</label>
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
                <label>方块搜索</label>
                <input v-model="blockSearch" placeholder="搜索方块或输入英文ID" />
                <label>方块</label>
                <input v-model="blockText" list="block-options" />
                <datalist id="block-options">
                  <option v-for="row in filteredBlocks" :key="row[0]" :value="row[1]"></option>
                </datalist>
                <label>类型</label>
                <select v-model="blockType"><option v-for="row in LIMIT_TYPES" :key="row[1]">{{ row[0] }}</option></select>
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
              <label>稀有度</label>
              <select v-model="form.rarity"><option v-for="row in RARITIES" :key="row[1]">{{ row[0] }}</option></select>
              <label>附魔光效</label>
              <select v-model="form.glint"><option>默认</option><option>开启</option><option>关闭</option></select>
              <span></span><label class="check-line"><input v-model="form.unbreakable" type="checkbox" />无法损坏</label>
              <span></span><label class="check-line"><input v-model="form.glider" type="checkbox" />鞘翅飞行</label>
              <span></span><label class="check-line"><input v-model="form.deathProtection" type="checkbox" />死亡保护</label>
              <span></span><label class="check-line"><input v-model="form.damageEnabled" type="checkbox" />启用当前损耗</label>
              <label>当前损耗</label><input v-model.number="form.damage" min="0" type="number" />
              <span></span><label class="check-line"><input v-model="form.maxDamageEnabled" type="checkbox" />启用最大耐久</label>
              <label>最大耐久</label><input v-model.number="form.maxDamage" min="1" type="number" />
              <span></span><label class="check-line"><input v-model="form.stackEnabled" type="checkbox" />启用最大堆叠</label>
              <label>最大堆叠</label><input v-model.number="form.maxStackSize" max="99" min="1" type="number" />
              <span></span><label class="check-line"><input v-model="form.repairEnabled" type="checkbox" />启用修复消耗</label>
              <label>修复消耗</label><input v-model.number="form.repairCost" min="0" type="number" />
              <label>隐藏组件</label><input v-model="form.hiddenComponents" />
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
                <span></span><label class="check-line"><input v-model="form.foodEnabled" type="checkbox" />启用食物</label>
                <label>营养值</label><input v-model.number="form.nutrition" min="0" type="number" />
                <label>饱和度</label><input v-model.number="form.saturation" min="0" step="0.001" type="number" />
                <label>总是可食用</label><select v-model="form.alwaysEat"><option>默认</option><option>是</option><option>否</option></select>
                <span></span><label class="check-line"><input v-model="form.consumableEnabled" type="checkbox" />启用消耗</label>
                <label>消耗时间</label><input v-model.number="form.consumeSeconds" min="0" step="0.001" type="number" />
                <label>消耗声音</label><input v-model="form.consumeSound" />
                <label>消耗粒子</label><select v-model="form.consumeParticles"><option>默认</option><option>是</option><option>否</option></select>
              </div>

              <EffectEditor
                v-else-if="foodToolTab === '食用效果'"
                v-model="form.consumeEffects"
                title="食用效果"
                @toast="showToast"
              />

              <div v-else class="table-tab">
                <div class="basic-grid tool-form">
                  <span></span><label class="check-line"><input v-model="form.toolEnabled" type="checkbox" />启用工具</label>
                  <label>默认挖掘速度</label><input v-model.number="form.defaultMiningSpeed" min="0" step="0.001" type="number" />
                  <label>每方块损耗</label><input v-model.number="form.damagePerBlock" min="0" type="number" />
                </div>
                <div class="inline-row">
                  <label>方块</label>
                  <input v-model="toolBlock" list="tool-block-options" />
                  <datalist id="tool-block-options">
                    <option v-for="row in BLOCKS" :key="row[0]" :value="row[1]"></option>
                  </datalist>
                  <label>速度</label>
                  <input v-model.number="toolRuleSpeed" min="0" step="0.001" type="number" />
                  <label>正确掉落</label>
                  <select v-model="toolCorrect"><option v-for="row in CORRECT_FOR_DROPS" :key="row[1]">{{ row[0] }}</option></select>
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
              <label>数据值</label><input v-model.number="form.bedrockDataValue" max="32767" min="0" type="number" />
              <label>物品锁</label>
              <select v-model="form.bedrockItemLock"><option v-for="row in ITEM_LOCK_MODES" :key="row[1]">{{ row[0] }}</option></select>
              <span></span><label class="check-line"><input v-model="form.bedrockKeepOnDeath" type="checkbox" />基岩死亡保留</label>
              <p class="sub-text">基岩版当前生成：基础 give、数据值、可放置、可破坏、物品锁、死亡保留</p>
            </div>
          </section>
        </Transition>
      </section>
    </section>

    <section class="card preview-card" :class="{ flash: rowFlash.preview }">
      <label>生成结果</label>
      <textarea id="preview" v-model="preview" spellcheck="false"></textarea>
    </section>

    <Transition name="toast">
      <div v-if="toastText" class="toast">{{ toastText }}</div>
    </Transition>

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
