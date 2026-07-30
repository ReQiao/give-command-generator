<script setup lang="ts">
/**
 * AI 模式面板：自然语言 → AI 意图 → 确定性命令 → 一键部署为 datapack。
 *
 * 语法安全性来自分层：AI 只产出「意图」，命令字符串由 logic/dispatch.ts 下经服务器
 * 实证的构建器生成，所以即便 AI 想歪了，也不会产出语法非法的命令。
 */
import { computed, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { buildSystemPrompt, parseAiContent } from "../logic/ai/prompt";
import { dispatchIntents } from "../logic/dispatch";
import type { GiveVersion } from "../logic/builder";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{ version: GiveVersion }>();
const emit = defineEmits<{ (e: "toast", message: string, duration?: number): void }>();

interface AiResponse {
  ok: boolean;
  content: string | null;
  error: string | null;
  balance: number;
  usage: { prompt: number; completion: number; total: number } | null;
}

interface SaveInfo {
  name: string;
  path: string;
}

interface DeployResult {
  packPath: string;
  commandCount: number;
  reloadCommand: string;
  runCommand: string;
}

const desktop = isTauri();

const userText = ref("");
const apiKey = ref("");
const busy = ref(false);
const errorText = ref("");
const explanation = ref("");
const commands = ref<string[]>([]);
/** AI 意图里构建失败的条目，单独列出而不是静默吞掉。 */
const failures = ref<string[]>([]);

const saves = ref<SaveInfo[]>([]);
const selectedSave = ref("");
const deployed = ref<DeployResult | null>(null);
const deploying = ref(false);

const canGenerate = computed(() => desktop && !busy.value && userText.value.trim().length > 0);
const canDeploy = computed(() => desktop && !deploying.value && commands.value.length > 0 && !!selectedSave.value);

const examples = [
  "做一把能射 TNT 的弓",
  "做一个丢在地上就会爆炸的地雷",
  "给我一把一刀秒杀的剑，名字叫「审判」",
  "召唤一只 40 血、拿钻石剑、不会动的僵尸 Boss",
];

async function generate() {
  if (!canGenerate.value) return;
  busy.value = true;
  errorText.value = "";
  explanation.value = "";
  commands.value = [];
  failures.value = [];
  deployed.value = null;

  try {
    const res = await invoke<AiResponse>("ai_generate", {
      systemPrompt: buildSystemPrompt(props.version),
      userText: userText.value.trim(),
      apiKey: apiKey.value.trim() || null,
    });

    if (!res.ok || !res.content) {
      errorText.value = res.error ?? "AI 调用失败。";
      return;
    }

    const parsed = parseAiContent(res.content);
    explanation.value = parsed.explanation;

    const results = dispatchIntents(parsed.intents, props.version);
    commands.value = results.map((r) => r.command).filter((c): c is string => Boolean(c));
    failures.value = results
      .filter((r) => r.error)
      .map((r) => `${r.intent.command}：${r.error}`);

    if (commands.value.length === 0) {
      errorText.value = "AI 没有产出可用的指令，换个说法再试试。";
    } else {
      emit("toast", `已生成 ${commands.value.length} 条指令`);
    }
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    busy.value = false;
  }
}

async function refreshSaves() {
  if (!desktop) return;
  try {
    saves.value = await invoke<SaveInfo[]>("datapack_list_saves", { savesDir: null });
    if (saves.value.length === 0) {
      emit("toast", "没有找到存档，可能 Minecraft 装在非默认位置", 3000);
    } else if (!selectedSave.value) {
      selectedSave.value = saves.value[0].path;
    }
  } catch (err) {
    errorText.value = `读取存档列表失败：${err instanceof Error ? err.message : String(err)}`;
  }
}

async function deploy() {
  if (!canDeploy.value) return;
  deploying.value = true;
  errorText.value = "";
  try {
    deployed.value = await invoke<DeployResult>("datapack_deploy", {
      savePath: selectedSave.value,
      commands: commands.value,
      version: props.version,
    });
    emit("toast", `已写入 ${deployed.value.commandCount} 条指令到存档`, 3000);
  } catch (err) {
    errorText.value = `部署失败：${err instanceof Error ? err.message : String(err)}`;
  } finally {
    deploying.value = false;
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
  <section class="card ai-card">
    <div v-if="!desktop" class="ai-notice">
      AI 生成与一键部署需要在桌面版里使用（浏览器里没有本地存档访问能力）。
    </div>

    <div class="ai-head">
      <span class="field-label">
        想要什么效果
        <InfoTip text="用大白话描述你想要的游戏内效果就行，不用管指令怎么写。例如「做一把能射 TNT 的弓」。" />
      </span>
      <div class="ai-key">
        <span class="field-label">
          API Key
          <InfoTip text="通义千问（阿里云 DashScope）的 API key。留空则读取环境变量 DASHSCOPE_API_KEY。key 只在本机使用，不会存进模板。" />
        </span>
        <input v-model="apiKey" type="password" placeholder="sk-..." autocomplete="off" />
      </div>
    </div>

    <textarea
      v-model="userText"
      class="ai-input"
      placeholder="例如：做一把能射 TNT 的弓"
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
      <button id="primary" type="button" :disabled="!canGenerate" @click="generate">
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

    <ul v-if="failures.length" class="ai-failures">
      <li v-for="(f, i) in failures" :key="i">跳过一条无法构建的意图 —— {{ f }}</li>
    </ul>

    <!-- 一键部署 -->
    <div v-if="commands.length" class="ai-deploy">
      <span class="field-label">
        部署到存档
        <InfoTip text="把上面这些指令打包成数据包写进存档，再在游戏里执行 /reload 加载。这是原版官方的分发方式，不涉及任何外挂手段。" />
      </span>
      <div class="ai-deploy-row">
        <select v-model="selectedSave" :disabled="saves.length === 0">
          <option v-if="saves.length === 0" value="">先点右边「扫描存档」</option>
          <option v-for="s in saves" :key="s.path" :value="s.path">{{ s.name }}</option>
        </select>
        <button type="button" :disabled="!desktop" @click="refreshSaves">扫描存档</button>
        <button id="primary" type="button" :disabled="!canDeploy" @click="deploy">
          {{ deploying ? "部署中…" : "一键部署" }}
        </button>
      </div>

      <div v-if="deployed" class="ai-deployed">
        <p>
          已写入 <strong>{{ deployed.commandCount }}</strong> 条指令。
          回到游戏里依次执行下面两条即可生效：
        </p>
        <div class="ai-deploy-cmds">
          <code>{{ deployed.reloadCommand }}</code>
          <button type="button" @click="copyText(deployed.reloadCommand, deployed.reloadCommand)">复制</button>
          <code>{{ deployed.runCommand }}</code>
          <button type="button" @click="copyText(deployed.runCommand, deployed.runCommand)">复制</button>
        </div>
        <p class="ai-deploy-path">{{ deployed.packPath }}</p>
      </div>
    </div>
  </section>
</template>
