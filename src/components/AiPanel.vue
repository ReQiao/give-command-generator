<script setup lang="ts">
/**
 * AI 模式面板 —— **录视频分支专用版本，和 main 上的不是一回事**。
 *
 * 语法安全性的来源不变：AI 只产出「意图」，命令字符串由经 mc-verifier 实证的
 * 确定性构建器生成，所以即便 AI 想歪了也不会产出语法非法的命令。变的是这套
 * 构建器住在哪：主线上它在服务器（server/src/give/），这个分支把它搬回了前端
 * （logic/dispatch.ts → logic/commands/*），这样不用起服务端、不用配环境变量，
 * 断网也能演示。
 *
 * 账号、灵魂币、充值、兑换码在这个分支里整块拿掉了——它们全都要服务器才有意义。
 * API key 由用户自己在界面上填，花的是他自己的钱。
 *
 * **不要合进 main。** 主线把构建器放在服务器是有代价换来的结论，
 * 见 src-tauri/src/ai.rs 顶部那两条理由。
 */
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { buildSystemPrompt, parseAiContent } from "../logic/ai/prompt";
import { dispatchIntents } from "../logic/dispatch";
import type { GiveVersion } from "../logic/builder";
import { desktop } from "../logic/auth";
import CustomSelect from "./CustomSelect.vue";
import DeployPanel from "./DeployPanel.vue";
import InfoTip from "./InfoTip.vue";

/**
 * active：当前是不是正显示在 AI 模式（由 App.vue 传 mode === 'ai' 进来）。
 * 面板本身用 v-show 常驻挂载（切模式不能丢掉用户已经填的内容/生成结果），
 * 所以点灯特效不能再靠"组件创建时机"触发一次——那样只有第一次切进 AI 模式
 * 才会放，之后来回切都不放了。改成监听这个 prop，每次从 false 变 true
 * （也就是每次切进 AI 模式）都重新点一次。
 */
const props = defineProps<{ version: GiveVersion; animate?: boolean; active?: boolean }>();
const emit = defineEmits<{
  (e: "toast", message: string, duration?: number): void;
  (e: "update:version", version: GiveVersion): void;
}>();

// ---------------- 启动特效 ----------------
// 切进 AI 模式时点亮「灵魂灯笼」：灰烬上浮 + 一道扫光。
// 纯 CSS 动画，期间面板照常可用，不挡任何操作。
const prefersReducedMotion =
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
const showIgnition = ref(false);
/**
 * 特效层已经落地、可以起跑了。
 *
 * 为什么要和 showIgnition 分成两个：`v-if` 插入 DOM 的那一帧，浏览器要同时
 * 完成「建元素 → 算样式 → 建合成层 → 栅格化那条渐变光带」，而 CSS 动画在元素
 * 一插入就开始跑了。于是最开始的三五帧全被这些一次性开销吃掉，看起来就是
 * 白线卡住不动。这里先只把节点放进去，隔两帧（一帧建层、一帧确认）再加
 * `running` 把动画点着，起跑时图层已经是热的。
 */
const igniteRunning = ref(false);
let igniteTimer: ReturnType<typeof setTimeout> | undefined;
let igniteRaf = 0;

/** 上浮的灰烬：位置/延时/时长/漂移各自随机，每次点燃都重新生成一遍，避免看出是同一套循环。 */
const embers = ref(
  Array.from({ length: 18 }, (_, i) => ({
    key: i,
    left: `${Math.random() * 100}%`,
    delay: `${Math.random() * 620}ms`,
    duration: `${1100 + Math.random() * 900}ms`,
    drift: `${(Math.random() - 0.5) * 60}px`,
    size: `${3 + Math.random() * 4}px`,
  })),
);

/** 点亮一次特效；关了界面动画、或系统要求减弱动效时直接不放。 */
function ignite() {
  if (props.animate === false || prefersReducedMotion) return;
  embers.value = embers.value.map((e) => ({
    ...e,
    left: `${Math.random() * 100}%`,
    delay: `${Math.random() * 620}ms`,
    duration: `${1100 + Math.random() * 900}ms`,
    drift: `${(Math.random() - 0.5) * 60}px`,
    size: `${3 + Math.random() * 4}px`,
  }));
  clearTimeout(igniteTimer);
  cancelAnimationFrame(igniteRaf);
  igniteRunning.value = false;
  showIgnition.value = true;
  igniteRaf = requestAnimationFrame(() => {
    igniteRaf = requestAnimationFrame(() => {
      if (showIgnition.value) igniteRunning.value = true;
    });
  });
  // 特效放完就把节点摘掉，别在 DOM 里留一层常驻的 overlay。
  igniteTimer = setTimeout(() => {
    showIgnition.value = false;
    igniteRunning.value = false;
  }, 2200);
}

if (props.active !== false) ignite(); // 面板一开始就是激活状态（比如刷新页面正停在 AI 模式）时，照常点一次

watch(
  () => props.active,
  (active, wasActive) => {
    if (active && !wasActive) ignite();
  },
);

interface AiResponse {
  ok: boolean;
  /** 一次性命令，可直接复制/一键部署——服务器已经跑完 dispatch+校验。 */
  commands: string[];
  /** 需要持续侦测的命令（execute 意图标了 loop:true 的）。 */
  loopCommands: string[];
  /** 构建失败的意图描述，格式 "${command}：${error}"。 */
  failures: string[];
  /** AI 给出的一句话说明。 */
  explanation: string;
  /** 顶层失败原因（没填 key / 上游调用失败 / 响应为空）。 */
  error: string | null;
  /** AI 返回的原始 JSON 文本，前端自己解析（parseAiContent → dispatchIntents）。 */
  content: string | null;
  usage: { prompt: number; completion: number; total: number } | null;
}

/**
 * 服务端说这个版本太旧就提示一次。对已经装着旧版的用户没用（这段代码在新版里），
 * 是给下一次不兼容变更留的通道——见 src-tauri/src/auth.rs::auth_upgrade_notice。
 */
async function checkUpgradeNotice() {
  if (!desktop) return;
  try {
    const notice = await invoke<string | null>("auth_upgrade_notice");
    if (notice) emit("toast", notice, 12000);
  } catch {
    // 拿不到就算了，不能因为版本检查失败挡住正常使用
  }
}

onMounted(() => {
  void checkUpgradeNotice();
  loadSettings();
});

/**
 * 大模型调用现在统一转发到自建服务器（key 只在服务器上，见
 * src-tauri/src/remote.rs 顶部注释），但模型选哪个仍然交给用户——价格/
 * 上下文/靠谱程度差很多（详见与用户的成本讨论）：Plus 最稳，Long 性价比
 * 最高，Flash 便宜但有 32k 阶梯计费跳档风险，Max/DeepSeek 贵但能力更强。
 * 留空/选不到就用服务器 .env 里 AI_MODEL 配置的默认值。
 */
const MODEL_OPTIONS = [
  { label: "Qwen Plus（稳）", value: "qwen-plus" },
  { label: "Qwen Max（旗舰，贵）", value: "qwen3.8-max" },
  { label: "Qwen Flash（便宜）", value: "qwen3.8-flash" },
  { label: "Qwen Long（长上下文）", value: "qwen-long-latest" },
  { label: "DeepSeek V4 Pro", value: "deepseek-v4-pro" },
] as const;
const apiModel = ref<string>(MODEL_OPTIONS[0].value);

/**
 * AI 服务商设置：端点 + 模型 + 用户自己的 API key。
 *
 * key 存在 localStorage 里而不是内存里——录视频要反复重启应用，每次重填很烦。
 * 它只留在这台机器上，不会发到任何地方（除了用户自己填的那个端点）。
 *
 * **绝对不要给它加"内置默认 key"兜底。** 那正是当年出事故的形态：
 * 一把 key 打进所有人的安装包，拆出来就能白嫖（见 src-tauri/src/ai.rs 顶部）。
 */
const KEY_STORE = "soul-lantern-demo-ai";
const PROVIDERS = [
  {
    label: "阿里云百炼（DashScope）",
    endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
  },
  { label: "OpenAI", endpoint: "https://api.openai.com/v1/chat/completions" },
  { label: "自定义", endpoint: "" },
] as const;

const apiKey = ref("");
const apiEndpoint = ref<string>(PROVIDERS[0].endpoint);
const showSettings = ref(false);

function loadSettings() {
  try {
    const raw = localStorage.getItem(KEY_STORE);
    if (!raw) return;
    const v = JSON.parse(raw) as { key?: string; endpoint?: string; model?: string };
    apiKey.value = v.key ?? "";
    if (v.endpoint) apiEndpoint.value = v.endpoint;
    if (v.model) apiModel.value = v.model;
  } catch {
    // 存坏了就当没存过，不能因为读配置失败挡住整个面板
  }
}

function saveSettings() {
  try {
    localStorage.setItem(
      KEY_STORE,
      JSON.stringify({ key: apiKey.value, endpoint: apiEndpoint.value, model: apiModel.value }),
    );
  } catch {
    // 无痕模式之类会抛，忽略——只是下次要重填
  }
}
watch([apiKey, apiEndpoint, apiModel], saveSettings);

/** 没填 key 时把「AI 生成指令」按钮也禁掉，别让人点了才发现。 */
const keyReady = computed(() => apiKey.value.trim().length > 0);

const userText = ref("");
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

/**
 * 基岩版暂不支持 AI 模式。
 *
 * 不是"还没做完"，是刻意不做：除了 give 以外的构建器（summon/setblock/effect/
 * enchant/scoreboard…）全都只会输出 Java 语法——基岩版的 /summon 根本没有 NBT
 * 参数，setblock 的方块状态写法也不一样，照 Java 那套生成出来的指令在基岩里是
 * 无效的。而 scripts/mc-verifier 跑的是 Java server.jar，整条工具链没有基岩
 * 服务端，基岩语法没法实测；本项目一贯只写实测过的语法（见 inGround/OnGround
 * 那次踩的坑），所以宁可明确挡住，也不给用户一堆看着像模像样、进游戏就报错的
 * 指令，更不该白扣他的灵魂币。手动模式的基岩 give 是经过 ID 表校对的，可以用。
 */
const bedrockUnsupported = computed(() => props.version === "bedrock");

const canGenerate = computed(
  () =>
    desktop &&
    !bedrockUnsupported.value &&
    !busy.value &&
    // 【录视频分支】没填 key 就别让人点了才发现——上面那块设置默认就是展开的
    keyReady.value &&
    userText.value.trim().length > 0,
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
      endpoint: apiEndpoint.value.trim() || null,
      model: apiModel.value.trim() || null,
      history: history.value,
    });

    if (!res.ok || !res.content) {
      errorText.value = res.error ?? "AI 调用失败。";
      return;
    }

    // 这个分支里解析和构建都在前端做（主线上是服务器做的）。
    // parseAiContent 只负责把 AI 的 JSON 变成意图数组、并挡掉非法项；
    // 真正的语法正确性由 dispatchIntents → logic/commands/* 保证。
    const parsed = parseAiContent(res.content);
    const results = dispatchIntents(parsed.intents, props.version);
    explanation.value = parsed.explanation;
    commands.value = results.filter((r) => r.command && !r.loop).map((r) => r.command as string);
    loopCommands.value = results.filter((r) => r.command && r.loop).map((r) => r.command as string);
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
    // rawContent 是服务器专供多轮历史使用的原始 AI 输出，不在 UI 展示。
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
  <section class="card ai-card" :class="{ igniting: igniteRunning }">
    <!-- 启动特效层：纯装饰，pointer-events:none，不拦任何点击。
         节点先插进来（showIgnition），隔两帧再加 running 起跑，见上面注释。 -->
    <div v-if="showIgnition" class="ai-ignite" :class="{ running: igniteRunning }" aria-hidden="true">
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

    <div v-if="bedrockUnsupported" class="ai-notice">
      <p><strong>AI 模式暂不支持基岩版。</strong></p>
      <p>
        AI 生成的指令由一套经过真实 Java 服务器实测的构建器产出，
        而基岩版的指令语法不一样（比如 <code>/summon</code> 在基岩版没有 NBT 参数），
        照 Java 那套生成出来在基岩版里是无效的。与其给你一堆看着像模像样、
        进游戏就报错的指令，不如先明确挡住。
      </p>
      <p>请切回<strong>手动模式</strong>使用基岩版的物品生成——那边的基岩 ID 是单独校对过的。</p>
    </div>

    <template v-else>
    <!-- 服务商设置。默认收起，只在没填 key 时自动展开——录制时不占画面，
         但第一次打开的人不会找不到入口。 -->
    <div class="ai-balance-bar">
      <span>
        AI 服务商
        <InfoTip text="这个版本直接用你自己的 API key 调用大模型，不经过任何服务器。key 只存在这台机器上，花的是你自己的额度。" />
        <strong>{{ keyReady ? "已配置" : "未配置" }}</strong>
      </span>
      <button type="button" class="ai-topup-toggle" @click="showSettings = !showSettings">
        {{ showSettings ? "收起" : "设置" }}
      </button>
    </div>

    <div v-if="showSettings || !keyReady" class="ai-topup-panel">
      <p class="ai-topup-note">
        key 不会发到除你所填端点之外的任何地方，也不会进仓库。这是本地演示版本。
      </p>
      <div class="ai-license">
        <span class="field-label">接口地址</span>
        <div class="ai-license-row">
          <input
            v-model="apiEndpoint"
            placeholder="https://…/v1/chat/completions"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        <div class="ai-topup-tiers">
          <button
            v-for="p in PROVIDERS.filter((x) => x.endpoint)"
            :key="p.label"
            type="button"
            class="ai-topup-tier"
            @click="apiEndpoint = p.endpoint"
          >
            <span class="ai-topup-coins">{{ p.label }}</span>
          </button>
        </div>
      </div>

      <div class="ai-license">
        <span class="field-label">API key</span>
        <div class="ai-license-row">
          <input
            v-model="apiKey"
            type="password"
            placeholder="sk-…"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      </div>
    </div>

    <div class="ai-head">
      <span class="field-label">
        想要什么效果
        <InfoTip text="用大白话描述你想要的游戏内效果就行，不用管指令怎么写。例如「做一把能射 TNT 的弓」。" />
      </span>
      <div class="ai-model-row">
        <span class="field-label">
          模型
          <InfoTip text="不同模型价格/能力差很多：Plus 最稳，Long 性价比最高，Flash 便宜但有 32k 阶梯计费跳档风险，Max/DeepSeek 贵但能力更强。选「服务器默认」就用服务器统一配置的那个。" />
        </span>
        <CustomSelect
          v-model="apiModel"
          :options="MODEL_OPTIONS as unknown as { label: string; value: string }[]"
        />
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

    <!-- datapack 是 Java 独有机制，基岩版没有——外层 v-else 已经挡住了，
         这里再挡一次是因为 DeployPanel 是和手动模式共用的组件，不该依赖
         "恰好被上层挡住了"才不出错（手动模式在 App.vue 那侧也有同样的守卫）。 -->
    <DeployPanel
      v-if="(commands.length || loopCommands.length) && version !== 'bedrock'"
      :commands="commands"
      :loop-commands="loopCommands"
      :version="version"
      @toast="(...args) => emit('toast', ...args)"
      @update:version="(v) => emit('update:version', v)"
    />
    </template>
  </section>
</template>
