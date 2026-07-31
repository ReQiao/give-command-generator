<script setup lang="ts">
/**
 * 存档部署面板：手动模式 / AI 模式共用。
 *
 * 存档定位两条路：
 *   1. 自动扫描 .minecraft/saves（走官方启动器的人多数能扫到）。
 *   2. 手动「浏览选择存档」——很多人不用官方启动器（PCL2/HMCL/多人合租的服主等），
 *      .minecraft 目录可能在任何地方，必须能让用户自己弹出系统文件夹选择框去挑，
 *      不能只靠自动扫描。
 *
 * commands 是一次性命令（部署后需要玩家手动执行一次 run_command），
 * loopCommands 是需要每 tick 持续侦测的命令（自动挂 tick.json，/reload 后即生效，
 * 不需要玩家再做任何事）——由调用方（AiPanel 的 execute loop:true 分流、
 * 或手动模式的单条 give）分别传入，允许只有一边非空。
 */
import { computed, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { GiveVersion } from "../logic/builder";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{
  commands: string[];
  loopCommands?: string[];
  version: GiveVersion;
}>();
const emit = defineEmits<{ (e: "toast", message: string, duration?: number): void }>();

interface SaveInfo {
  name: string;
  path: string;
}

interface DeployResult {
  packPath: string;
  commandCount: number;
  loopCommandCount: number;
  reloadCommand: string;
  runCommand: string | null;
}

const desktop = isTauri();
const loopCommands = computed(() => props.loopCommands ?? []);

const saves = ref<SaveInfo[]>([]);
const selectedSave = ref("");
const deployed = ref<DeployResult | null>(null);
const deploying = ref(false);
const errorText = ref("");

const canDeploy = computed(
  () =>
    desktop &&
    !deploying.value &&
    (props.commands.length > 0 || loopCommands.value.length > 0) &&
    !!selectedSave.value,
);

async function refreshSaves() {
  if (!desktop) return;
  try {
    saves.value = await invoke<SaveInfo[]>("datapack_list_saves", { savesDir: null });
    if (saves.value.length === 0) {
      emit("toast", "没有找到存档，可能 Minecraft 装在非默认位置，试试右边「浏览选择存档」", 3500);
    } else if (!selectedSave.value) {
      selectedSave.value = saves.value[0].path;
    }
  } catch (err) {
    errorText.value = `读取存档列表失败：${err instanceof Error ? err.message : String(err)}`;
  }
}

/**
 * 手动浏览选择存档文件夹——不依赖自动扫描，适合非官方启动器（.minecraft 位置各异）的用户。
 * 弹出系统原生文件夹选择框，尽量以 .minecraft/saves 为起点，方便用户直接进去挑某个存档。
 */
async function browseSave() {
  if (!desktop) return;
  try {
    const defaultPath = (await invoke<string | null>("datapack_default_saves_dir")) ?? undefined;
    const picked = await open({
      directory: true,
      multiple: false,
      defaultPath,
      title: "选择一个 Minecraft 存档文件夹（saves 里的某一个世界）",
    });
    if (typeof picked !== "string") return; // 用户取消
    selectedSave.value = picked;
    const name = picked.split(/[\\/]/).filter(Boolean).pop() ?? picked;
    if (!saves.value.some((s) => s.path === picked)) {
      saves.value = [...saves.value, { name, path: picked }];
    }
    emit("toast", `已选择存档：${name}`);
  } catch (err) {
    errorText.value = `选择存档失败：${err instanceof Error ? err.message : String(err)}`;
  }
}

async function deploy() {
  if (!canDeploy.value) return;
  deploying.value = true;
  errorText.value = "";
  try {
    deployed.value = await invoke<DeployResult>("datapack_deploy", {
      savePath: selectedSave.value,
      commands: props.commands,
      loopCommands: loopCommands.value,
      version: props.version,
    });
    emit(
      "toast",
      `已写入 ${deployed.value.commandCount + deployed.value.loopCommandCount} 条指令到存档`,
      3000,
    );
  } catch (err) {
    errorText.value = `部署失败：${err instanceof Error ? err.message : String(err)}`;
  } finally {
    deploying.value = false;
  }
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    emit("toast", `已复制 ${text}`);
  } catch {
    emit("toast", "复制失败，请手动复制");
  }
}
</script>

<template>
  <div class="ai-deploy">
    <span class="field-label">
      部署到存档
      <InfoTip text="把生成的指令打包成数据包写进存档，回到游戏里执行 /reload 加载。这是原版官方的分发方式，不涉及任何外挂手段。需要持续侦测的效果（循环命令）会自动挂到 tick 循环上，/reload 后立刻生效，不用再手动放命令方块。" />
    </span>
    <div class="ai-deploy-row">
      <select v-model="selectedSave" :disabled="saves.length === 0">
        <option v-if="saves.length === 0" value="">还没选存档</option>
        <option v-for="s in saves" :key="s.path" :value="s.path">{{ s.name }}</option>
      </select>
      <button type="button" :disabled="!desktop" @click="refreshSaves">扫描存档</button>
      <button type="button" :disabled="!desktop" @click="browseSave">浏览选择存档…</button>
      <button class="primary-btn" type="button" :disabled="!canDeploy" @click="deploy">
        {{ deploying ? "部署中…" : "一键部署" }}
      </button>
    </div>
    <p class="deploy-hint">
      不用官方启动器？点「浏览选择存档」直接在 saves 文件夹里选一个存档即可，不依赖自动扫描。
    </p>

    <p v-if="errorText" class="ai-error">{{ errorText }}</p>

    <div v-if="deployed" class="ai-deployed">
      <p v-if="deployed.loopCommandCount > 0">
        已写入 <strong>{{ deployed.loopCommandCount }}</strong> 条循环侦测命令，已自动挂到 tick 循环。
        回到游戏里执行下面这条 <strong>/reload</strong> 即可生效，不需要再放命令方块。
      </p>
      <p v-else>
        已写入 <strong>{{ deployed.commandCount }}</strong> 条指令。
        回到游戏里依次执行下面命令即可生效：
      </p>
      <div class="ai-deploy-cmds">
        <code>{{ deployed.reloadCommand }}</code>
        <button type="button" @click="copyText(deployed.reloadCommand)">复制</button>
        <template v-if="deployed.runCommand">
          <code>{{ deployed.runCommand }}</code>
          <button type="button" @click="copyText(deployed.runCommand)">复制</button>
        </template>
      </div>
      <p class="ai-deploy-path">{{ deployed.packPath }}</p>
    </div>
  </div>
</template>
