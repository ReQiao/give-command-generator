<script setup lang="ts">
/**
 * AI 模式面板：自然语言 → 确定性命令 → 一键部署为 datapack。
 *
 * 语法安全性来自分层：AI 只产出「意图」，命令字符串由服务器上经 mc-verifier
 * 实证的确定性构建器生成（server/src/give/），所以即便 AI 想歪了，也不会产出
 * 语法非法的命令。这套构建逻辑此前跑在这个客户端里，现在已经搬到服务器——
 * 它才是这个项目真正的护城河，比服务器保管的 API key 更值得保护，留在客户端
 * 容易被逆向。搬迁后这个面板只管展示服务器返回的结果，不再自己解析 AI 输出、
 * 不再自己校验目录、不再自己拼指令字符串。
 */
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { buildSystemPrompt } from "../logic/ai/prompt";
import type { GiveVersion } from "../logic/builder";
// 登录态是全局单一真相源（见 logic/auth.ts 顶部注释）：门禁判断现在也发生在
// App.vue 的模式切换按钮上，两处各存一份 ref 会立刻不同步。
import {
  auth,
  desktop,
  gated,
  logout as doLogout,
  openAuth,
  recheckAuth,
} from "../logic/auth";
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
  /** 顶层失败原因（余额不足/上游调用失败/AI 内容解析失败）。 */
  error: string | null;
  /**
   * 现在走远程服务器代理（真实大模型 key + Builder 构建逻辑都只在服务器上，
   * 见 src-tauri/src/remote.rs 顶部注释——这个客户端曾经把 key 直接编译进
   * 安装包，被人拆出来盗刷过一次）。连不上服务器时是 null，不能瞎填 0——
   * 那会让用户误以为余额真的清零了。
   */
  balance: number | null;
  usage: { prompt: number; completion: number; total: number } | null;
  /** 供本组件存入多轮对话历史使用，不在 UI 展示。 */
  rawContent: string | null;
}

/**
 * 灵魂币余额 + 充值。真实扣费在后端（ai_generate 成功后按用量折算），
 * 这里只是展示余额和发起充值——充值现在是免费直接加余额（billing_recharge
 * 还没接真实支付网关），等真要收钱时只用换后端实现，这几个调用点不用动。
 */
interface AccountView {
  activated: boolean;
  balance: number;
}
interface TopupTier {
  yuan: number;
  coins: number;
}
const balance = ref<number | null>(null);
const topupTiers = ref<TopupTier[]>([]);
const showTopup = ref(false);

// refreshAuth 现在住在 logic/auth.ts 里，它不知道余额条这回事，
// 所以余额跟随登录态的同步放在这里做。
watch(
  () => auth.value.loggedIn,
  (loggedIn) => {
    if (loggedIn) balance.value = auth.value.balance;
  },
  { immediate: true },
);

async function logout() {
  await doLogout();
  balance.value = null;
  emit("toast", "已退出登录");
}

async function refreshBalance() {
  if (!desktop) return;
  try {
    const st = await invoke<AccountView>("billing_state");
    balance.value = st.balance;
  } catch {
    // 读不到就算了，不影响主流程；余额会在下次生成成功后从 AiResponse 里更新。
  }
}

async function loadTopupTiers() {
  if (!desktop) return;
  try {
    topupTiers.value = await invoke<TopupTier[]>("billing_topup_tiers");
  } catch {
    topupTiers.value = [];
  }
}

async function recharge(coins: number) {
  try {
    const st = await invoke<AccountView>("billing_recharge", { coins });
    balance.value = st.balance;
    emit("toast", `已充值 ${coins} 灵魂币（当前免费测试阶段，未实际扣款）`);
  } catch (err) {
    emit("toast", `充值失败：${err instanceof Error ? err.message : String(err)}`);
    // 失败多半是会话过期/根本没登录。回头刷一次登录态，让登录入口重新出现——
    // 否则用户看到"请重新登录"却在界面上找不到任何地方能登录。
    void recheckAuth();
  }
}

/**
 * 激活码兑换。
 *
 * 这段注释以前写的是"后端只做格式校验、没有真实性验证，别暗示这是一道真防线"——
 * 那已经过时了，现在三条全都做到了：激活码带 HMAC 校验位（用只存在于服务器
 * 环境变量里的 pepper 算，见 server/src/crypto.rs::verify_license）、全局一次性
 * 核销、10 次/24h 的爆破限流。改造前那套只校验格式，等于约 36^12 个字符串
 * 每一个都是能兑 100 币的一次性券，写个 for 循环就能刷。
 *
 * 客户端这边**只做"长得像不像"的即时提示，不判断有效性**——校验位客户端既
 * 算不出也不该算得出，否则伪造能力就跟着安装包分发出去了。
 */
const licenseKey = ref("");
const activating = ref(false);

async function activate() {
  const key = licenseKey.value.trim();
  if (!key || activating.value) return;
  activating.value = true;
  try {
    const st = await invoke<AccountView>("billing_activate", { licenseKey: key });
    balance.value = st.balance;
    licenseKey.value = "";
    emit("toast", "激活码已兑换");
  } catch (err) {
    emit("toast", `兑换失败：${err instanceof Error ? err.message : String(err)}`);
  } finally {
    activating.value = false;
  }
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
  // 登录态/门禁开关由 App.vue 在挂载时统一拉一次（模式切换按钮要用），这里不重复。
  void refreshBalance();
  void loadTopupTiers();
});

/**
 * 大模型调用现在统一转发到自建服务器（key 只在服务器上，见
 * src-tauri/src/remote.rs 顶部注释），但模型选哪个仍然交给用户——价格/
 * 上下文/靠谱程度差很多（详见与用户的成本讨论）：Plus 最稳，Long 性价比
 * 最高，Flash 便宜但有 32k 阶梯计费跳档风险，Max/DeepSeek 贵但能力更强。
 * 留空/选不到就用服务器 .env 里 AI_MODEL 配置的默认值。
 */
const MODEL_OPTIONS = [
  { label: "服务器默认", value: "" },
  { label: "Qwen Plus（稳）", value: "qwen3.7-plus" },
  { label: "Qwen Max（旗舰，贵）", value: "qwen3.8-max" },
  { label: "Qwen Flash（最便宜，注意32k阶梯跳价）", value: "qwen3.7-flash" },
  { label: "Qwen Long（长上下文，性价比高）", value: "qwen-long-latest" },
  { label: "DeepSeek V4 Pro", value: "deepseek-v4-pro" },
] as const;
const apiModel = ref<string>("");

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
  () => desktop && !bedrockUnsupported.value && !busy.value && userText.value.trim().length > 0,
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
      model: apiModel.value.trim() || null,
      version: props.version,
      history: history.value,
    });

    // 连不上服务器时 res.balance 是 null，不能拿它覆盖已经显示的余额——
    // 那会让用户误以为余额真的清零了，其实只是网络问题。
    if (res.balance !== null) balance.value = res.balance;

    if (!res.ok) {
      errorText.value = res.error ?? "AI 调用失败。";
      return;
    }

    // 解析/校验/构建现在全部在服务器上完成（server/src/give/），这里直接
    // 用服务器已经分好类的结果，不用再自己解析 AI 输出、跑目录校验。
    explanation.value = res.explanation;
    commands.value = res.commands;
    loopCommands.value = res.loopCommands;
    failures.value = res.failures;

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
      { role: "assistant", content: res.rawContent ?? "" },
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
  <!-- ignite-flat 跟着 showIgnition 走（比 igniting 早两帧）：它负责在特效期间
       摘掉卡片的 backdrop-filter，那一下会让整张卡重绘，必须发生在预热窗口里，
       不能和动画起跑撞在同一帧。原因见 style.css 里 .ai-card.ignite-flat 的注释。 -->
  <!-- data-glass-off 是给 logic/glass.ts 看的：它写的是内联 backdrop-filter，
       优先级压过 CSS，所以 .ignite-flat 那条 `backdrop-filter: none` 单靠 CSS
       生效不了，必须让 glass.ts 主动把内联样式清掉。少了这个属性，点灯动画
       会从 60 帧掉回 16 帧（实测过）。 -->
  <section
    class="card ai-card"
    :class="{ 'ignite-flat': showIgnition, igniting: igniteRunning }"
    :data-glass-off="showIgnition ? '1' : undefined"
  >
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

    <!-- 登录门禁：和上面 bedrockUnsupported 那块用同一个范式（.ai-notice 黄条整块挡住），
         这个写法这个文件里本来就有，直接复用不另起炉灶。
         注意门禁写在 v-else 内部 —— 基岩版那条提示优先级更高，
         选了基岩版就该先说"AI 模式不支持基岩版"，而不是先要求登录。 -->
    <div v-else-if="gated" class="ai-notice ai-gate">
      <p><strong>AI 模式需要先登录。</strong></p>
      <p>
        AI 生成会真实调用大模型、按用量计费，所以需要一个账号来记余额——
        账号也让你换电脑、重装之后余额还在。注册只要用户名、密码和一个手机号。
      </p>
      <p v-if="auth.offline" class="ai-gate-offline">
        （现在连不上服务器，可能是网络问题或者服务器在维护，稍后再试试。）
      </p>
      <div class="ai-gate-actions">
        <button class="primary-btn" type="button" @click="openAuth('login')">登录 / 注册</button>
      </div>
    </div>

    <template v-else>
    <div v-if="desktop" class="ai-balance-bar">
      <span>
        灵魂币余额：<strong>{{ balance ?? "—" }}</strong>
        <InfoTip text="每次 AI 生成会按真实调用花费（不同模型单价不同）折算扣除灵魂币，不是固定扣一个数。当前充值是免费测试阶段，不会真的扣款。" />
      </span>
      <span v-if="auth.loggedIn" class="ai-account">
        {{ auth.username }}
        <button type="button" class="auth-link" @click="openAuth('change')">修改密码</button>
        <button type="button" class="auth-link" @click="logout">退出登录</button>
      </span>
      <!-- 未登录时这里必须有入口。走到这个分支说明 gated 是 false，
           也就是门禁块（唯一另一个登录按钮所在处）没渲染——少了这个 v-else，
           用户就会卡在"点充值报请登录、但界面上找不到哪里能登录"的死胡同里。 -->
      <span v-else class="ai-account">
        <button type="button" class="auth-link" @click="openAuth('login')">登录 / 注册</button>
      </span>
      <button type="button" class="ai-topup-toggle" @click="showTopup = !showTopup">
        充值
      </button>
    </div>

    <div v-if="showTopup" class="ai-topup-panel">
      <p class="ai-topup-note">当前是免费测试阶段，点击即可直接到账，不会真的扣款。</p>
      <div class="ai-topup-tiers">
        <button
          v-for="tier in topupTiers"
          :key="tier.coins"
          type="button"
          class="ai-topup-tier"
          @click="recharge(tier.coins)"
        >
          <span class="ai-topup-yuan">¥{{ tier.yuan }}</span>
          <span class="ai-topup-coins">{{ tier.coins }} 灵魂币</span>
        </button>
      </div>

      <div class="ai-license">
        <span class="field-label">
          激活码
          <InfoTip text="在外部渠道购买后拿到的激活码，格式 SOUL-XXXX-XXXX-XXXX。同一个码只能兑换一次。" />
        </span>
        <div class="ai-license-row">
          <input
            v-model="licenseKey"
            placeholder="SOUL-XXXX-XXXX-XXXX"
            autocomplete="off"
            spellcheck="false"
            @keydown.enter="activate"
          />
          <button type="button" :disabled="!licenseKey.trim() || activating" @click="activate">
            {{ activating ? "兑换中…" : "兑换" }}
          </button>
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
