<script setup lang="ts">
/**
 * AI 模式面板：自然语言 → AI 意图 → 确定性命令 → 一键部署为 datapack。
 *
 * 语法安全性来自分层：AI 只产出「意图」，命令字符串由 logic/dispatch.ts 下经服务器
 * 实证的构建器生成，所以即便 AI 想歪了，也不会产出语法非法的命令。
 */
import { computed, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { buildSystemPrompt, parseAiContent } from "../logic/ai/prompt";
import { dispatchIntents } from "../logic/dispatch";
import type { GiveVersion } from "../logic/builder";
import CustomSelect from "./CustomSelect.vue";
import DeployPanel from "./DeployPanel.vue";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{ version: GiveVersion; animate?: boolean }>();
const emit = defineEmits<{
  (e: "toast", message: string, duration?: number): void;
  (e: "update:version", version: GiveVersion): void;
}>();

// ---------------- 启动特效 ----------------
// 切进 AI 模式时点亮「灵魂灯笼」：灰烬上浮 + 一道扫光。
// 纯 CSS 动画，只在挂载时放一次；期间面板照常可用，不挡任何操作。
const prefersReducedMotion =
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
/** 关掉界面动画、或系统要求减弱动效时，直接不渲染特效层。 */
const showIgnition = ref(props.animate !== false && !prefersReducedMotion);

/** 上浮的灰烬：位置/延时/时长/漂移各自随机，避免看出是同一套循环。 */
const embers = Array.from({ length: 18 }, (_, i) => ({
  key: i,
  left: `${Math.random() * 100}%`,
  delay: `${Math.random() * 620}ms`,
  duration: `${1100 + Math.random() * 900}ms`,
  drift: `${(Math.random() - 0.5) * 60}px`,
  size: `${3 + Math.random() * 4}px`,
}));

if (showIgnition.value) {
  // 特效放完就把节点摘掉，别在 DOM 里留一层常驻的 overlay。
  setTimeout(() => {
    showIgnition.value = false;
  }, 2200);
}

interface AiResponse {
  ok: boolean;
  content: string | null;
  error: string | null;
  balance: number;
  usage: { prompt: number; completion: number; total: number } | null;
}

const desktop = isTauri();

/**
 * 后端调用的是通用的 OpenAI 兼容 chat/completions 接口，不绑定某一家服务商。
 * 这里只是给几个常见服务商预填接口地址 + 默认模型，方便切换；选「自定义」时
 * 两个输入框保持可编辑，填其他任何 OpenAI 兼容服务（接口地址需带完整路径）都行。
 */
const PROVIDER_PRESETS = {
  dashscope: {
    label: "通义千问 Qwen（DashScope）",
    endpoint: "https://ws-b2ui8x9tozwc8cq1.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/chat/completions",
    model: "qwen-plus",
  },
  openai: {
    label: "OpenAI",
    endpoint: "https://api.openai.com/v1/chat/completions",
    model: "gpt-4o-mini",
  },
  custom: {
    label: "自定义 / 其他 OpenAI 兼容接口",
    endpoint: "",
    model: "",
  },
} as const;
type Provider = keyof typeof PROVIDER_PRESETS;

/**
 * dashscope 这个工作空间接口下挂了好几个模型，价格/上下文/靠谱程度差很多
 * （详见与用户的成本讨论）：Plus 最稳，Long 性价比最高，Flash 便宜但有 32k
 * 阶梯计费跳档风险，Max/DeepSeek 贵但能力更强，谨慎使用。
 * 模型名是按控制台上显示的猜的，如果调不通以「自定义」输入框手动改。
 */
const MODEL_OPTIONS = [
  { label: "Qwen Plus（默认，稳）", value: "qwen3.7-plus" },
  { label: "Qwen Max（旗舰，贵）", value: "qwen3.8-max" },
  { label: "Qwen Flash（最便宜，注意32k阶梯跳价）", value: "qwen3.7-flash" },
  { label: "Qwen Long（长上下文，性价比高）", value: "qwen-long-latest" },
  { label: "DeepSeek V4 Pro", value: "deepseek-v4-pro" },
] as const;

const provider = ref<Provider>("dashscope");
const providerOptions = (Object.keys(PROVIDER_PRESETS) as Provider[]).map((value) => ({
  label: PROVIDER_PRESETS[value].label,
  value,
}));
const apiBase = ref<string>(PROVIDER_PRESETS.dashscope.endpoint);
const apiModel = ref<string>(PROVIDER_PRESETS.dashscope.model);

watch(provider, (value) => {
  const preset = PROVIDER_PRESETS[value];
  apiBase.value = preset.endpoint;
  apiModel.value = preset.model;
});

const userText = ref("");
const apiKey = ref("");
const busy = ref(false);
const errorText = ref("");
const explanation = ref("");
/** 一次性命令：可以直接复制粘贴到聊天栏，也可以走一键部署。 */
const commands = ref<string[]>([]);
/**
 * 需要每 tick 持续侦测的命令（execute 意图标了 loop:true 的）。
 * 这类命令单独复制粘贴到聊天栏没有意义——原版没有"持续执行"这回事，必须要么
 * 手动放进循环命令方块，要么部署成 datapack（自动挂 tick.json）。所以不给
 * 每条配复制按钮，只展示内容 + 明确提示走一键部署。
 */
const loopCommands = ref<string[]>([]);
/** AI 意图里构建失败的条目，单独列出而不是静默吞掉。 */
const failures = ref<string[]>([]);

/**
 * 多轮上下文：允许"在上一次生成结果基础上继续修改"（比如"改成用箭"），
 * 而不用重新把整句需求描述一遍。封顶3轮是刻意的——通义千问上下文有限，
 * 轮数越多越容易跑偏/幻觉，而且接口是无状态的，每轮都要把历史重新整个
 * 发一遍，轮数越多单次调用费的 token 越多，3轮是防幻觉和控成本的折中。
 */
const MAX_CONTEXT_ROUNDS = 3;
interface ChatTurn {
  role: "user" | "assistant";
  content: string;
}
const history = ref<ChatTurn[]>([]);
const round = computed(() => Math.floor(history.value.length / 2));
const isContinuing = computed(() => history.value.length > 0);

/** 手动清空上下文，开始全新的一轮需求描述。 */
function newConversation() {
  history.value = [];
}

const canGenerate = computed(
  () => desktop && !busy.value && userText.value.trim().length > 0 && apiBase.value.trim().length > 0,
);

const examples = [
  "做一把能射 TNT 的弓",
  "做一个丢在地上就会爆炸的地雷",
  "给我一把一刀秒杀的剑，名字叫「审判」",
  "召唤一只 40 血、拿钻石剑、不会动的僵尸 Boss",
];

async function generate() {
  if (!canGenerate.value) return;

  // 已经聊满3轮：这一次不再带历史，直接当新对话处理，而不是拒绝用户的请求。
  if (round.value >= MAX_CONTEXT_ROUNDS) {
    history.value = [];
    emit("toast", `已达到连续对话上限（${MAX_CONTEXT_ROUNDS}轮），这次将开始新的对话`);
  }

  busy.value = true;
  errorText.value = "";
  explanation.value = "";
  commands.value = [];
  loopCommands.value = [];
  failures.value = [];

  const thisTurnText = userText.value.trim();

  try {
    const res = await invoke<AiResponse>("ai_generate", {
      systemPrompt: buildSystemPrompt(props.version),
      userText: thisTurnText,
      apiKey: apiKey.value.trim() || null,
      endpoint: apiBase.value.trim() || null,
      model: apiModel.value.trim() || null,
      history: history.value,
    });

    if (!res.ok || !res.content) {
      errorText.value = res.error ?? "AI 调用失败。";
      return;
    }

    const parsed = parseAiContent(res.content);
    explanation.value = parsed.explanation;

    const results = dispatchIntents(parsed.intents, props.version);
    const ok = results.filter((r): r is typeof r & { command: string } => Boolean(r.command));
    commands.value = ok.filter((r) => !r.loop).map((r) => r.command);
    loopCommands.value = ok.filter((r) => r.loop).map((r) => r.command);
    failures.value = results
      .filter((r) => r.error)
      .map((r) => `${r.intent.command}：${r.error}`);

    const total = commands.value.length + loopCommands.value.length;
    if (total === 0) {
      errorText.value = "AI 没有产出可用的指令，换个说法再试试。";
    } else {
      emit("toast", `已生成 ${total} 条指令`);
    }

    // 只在成功拿到回复后才计入历史——调用失败/解析失败不该污染上下文。
    history.value = [
      ...history.value,
      { role: "user", content: thisTurnText },
      { role: "assistant", content: res.content },
    ];
    userText.value = "";
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    busy.value = false;
  }
}

async function copyText(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text);
    emit("toast", `已复制${label}`);
  } catch {
    emit("toast", "复制失败，请手动复制");
  }
}

function copyAll() {
  void copyText(commands.value.join("\n"), `全部 ${commands.value.length} 条指令`);
}
</script>

<template>
  <section class="card ai-card" :class="{ igniting: showIgnition }">
    <!-- 启动特效层：纯装饰，pointer-events:none，不拦任何点击 -->
    <div v-if="showIgnition" class="ai-ignite" aria-hidden="true">
      <span class="ai-sweep"></span>
      <span
        v-for="e in embers"
        :key="e.key"
        class="ai-ember"
        :style="{
          left: e.left,
          width: e.size,
          height: e.size,
          animationDelay: e.delay,
          animationDuration: e.duration,
          '--drift': e.drift,
        }"
      ></span>
    </div>

    <div v-if="!desktop" class="ai-notice">
      AI 生成与一键部署需要在桌面版里使用（浏览器里没有本地存档访问能力）。
    </div>

    <div class="ai-head">
      <span class="field-label">
        想要什么效果
        <InfoTip text="用大白话描述你想要的游戏内效果就行，不用管指令怎么写。例如「做一把能射 TNT 的弓」。" />
      </span>
      <div class="ai-provider-row">
        <div class="ai-key">
          <span class="field-label">
            服务商
            <InfoTip text="后端调用的是通用的 OpenAI 兼容接口，不绑定某一家。选一个预设会自动填接口地址和模型，也可以选「自定义」接入其他 OpenAI 兼容服务。" />
          </span>
          <CustomSelect v-model="provider" :options="providerOptions" />
        </div>
        <div class="ai-key">
          <span class="field-label">
            接口地址
            <InfoTip text="OpenAI 兼容的 chat/completions 接口完整地址。切换服务商预设会自动填好，也可以手动改。" />
          </span>
          <input v-model="apiBase" placeholder="https://.../v1/chat/completions" autocomplete="off" />
        </div>
        <div class="ai-key">
          <span class="field-label">
            模型
            <InfoTip text="要调用的模型名称。DashScope 预设下拉可选常用几个；选其他服务商，或下拉里没有的模型名，手动填。" />
          </span>
          <CustomSelect
            v-if="provider === 'dashscope'"
            v-model="apiModel"
            :options="MODEL_OPTIONS as unknown as { label: string; value: string }[]"
          />
          <input v-else v-model="apiModel" placeholder="模型名" autocomplete="off" />
        </div>
        <div class="ai-key">
          <span class="field-label">
            API Key
            <InfoTip text="所选服务商的 API key。留空则读取环境变量 AI_API_KEY。key 只在本机使用，不会存进模板。" />
          </span>
          <input v-model="apiKey" type="password" placeholder="sk-..." autocomplete="off" />
        </div>
      </div>
    </div>

    <div v-if="isContinuing" class="ai-context-bar">
      <span>
        继续对话中（{{ round }}/{{ MAX_CONTEXT_ROUNDS }} 轮）
        <InfoTip text="接下来生成会带上前面几轮的对话，可以直接说「改成用箭」这种追问式修改。超过3轮后会自动开始新对话（防止上下文太长跑偏、也控制费用）。" />
      </span>
      <button type="button" class="ai-new-chat" @click="newConversation">开始新对话</button>
    </div>

    <textarea
      v-model="userText"
      class="ai-input"
      :placeholder="isContinuing ? '继续追问，例如：改成用箭' : '例如：做一把能射 TNT 的弓'"
      spellcheck="false"
      @keydown.ctrl.enter="generate"
    ></textarea>

    <div class="ai-examples">
      <span class="ai-examples-label">试试：</span>
      <button v-for="ex in examples" :key="ex" class="ai-chip" type="button" @click="userText = ex">
        {{ ex }}
      </button>
    </div>

    <div class="ai-actions">
      <button class="primary-btn" type="button" :disabled="!canGenerate" @click="generate">
        {{ busy ? "生成中…" : "AI 生成指令" }}
      </button>
      <button type="button" :disabled="commands.length === 0" @click="copyAll">复制全部</button>
    </div>

    <p v-if="errorText" class="ai-error">{{ errorText }}</p>

    <div v-if="explanation" class="ai-explain">
      <strong>思路：</strong>{{ explanation }}
    </div>

    <ul v-if="commands.length" class="ai-results">
      <li v-for="(cmd, i) in commands" :key="i">
        <code>{{ cmd }}</code>
        <button type="button" @click="copyText(cmd, '这条指令')">复制</button>
      </li>
    </ul>

    <!-- 循环侦测命令：单条复制粘贴到聊天栏没有意义（原版没有"持续执行"这回事），
         不给复制按钮，只展示内容并明确要求走一键部署。 -->
    <div v-if="loopCommands.length" class="ai-loop-block">
      <p class="ai-loop-hint">
        以下 {{ loopCommands.length }} 条需要持续侦测，无法通过复制粘贴生效——必须用下面的「一键部署」，
        会自动挂到数据包的 tick 循环上，/reload 后自动生效：
      </p>
      <ul class="ai-results ai-loop-results">
        <li v-for="(cmd, i) in loopCommands" :key="i"><code>{{ cmd }}</code></li>
      </ul>
    </div>

    <ul v-if="failures.length" class="ai-failures">
      <li v-for="(f, i) in failures" :key="i">跳过一条无法构建的意图 —— {{ f }}</li>
    </ul>

    <DeployPanel
      v-if="commands.length || loopCommands.length"
      :commands="commands"
      :loop-commands="loopCommands"
      :version="version"
      @toast="(...args) => emit('toast', ...args)"
      @update:version="(v) => emit('update:version', v)"
    />
  </section>
</template>
