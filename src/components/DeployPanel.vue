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
import { computed, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { detectGiveVersionFromRaw, type GiveVersion } from "../logic/builder";
import { VERSIONS } from "../data/catalog";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{
  commands: string[];
  loopCommands?: string[];
  version: GiveVersion;
}>();
const emit = defineEmits<{
  (e: "toast", message: string, duration?: number): void;
  (e: "update:version", version: GiveVersion): void;
}>();

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

/**
 * 存档实际版本识别：读 level.dat 的 Data.Version.Name，跟当前选择的版本比对，
 * 不一致就提示一下——纯提示，不强行覆盖用户的选择（用户可能就是故意要按某个
 * 目标版本生成，哪怕和存档当前版本不一样，比如提前给要升级的存档准备指令）。
 */
const detectedRawVersion = ref<string | null>(null);
const detectedVersion = computed(() =>
  detectedRawVersion.value ? detectGiveVersionFromRaw(detectedRawVersion.value) : null,
);
const detectedVersionLabel = computed(() => {
  const found = VERSIONS.find(([, value]) => value === detectedVersion.value);
  return found?.[0] ?? detectedRawVersion.value ?? "";
});
const versionMismatch = computed(
  () => !!detectedVersion.value && detectedVersion.value !== props.version,
);

watch(selectedSave, async (path) => {
  detectedRawVersion.value = null;
  if (!desktop || !path) return;
  try {
    detectedRawVersion.value = await invoke<string | null>("datapack_detect_version", { savePath: path });
  } catch {
    // 识别失败（存档损坏/老版本 level.dat 结构不同）静默忽略，不阻塞部署流程。
  }
});

function useDetectedVersion() {
  if (detectedVersion.value) emit("update:version", detectedVersion.value);
}

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

    <p v-if="versionMismatch" class="deploy-version-mismatch">
      检测到这个存档的实际版本是 <strong>{{ detectedVersionLabel }}</strong>，
      和当前选择的版本不一样，生成的指令可能对不上这个存档的语法。
      <button type="button" @click="useDetectedVersion">改用检测到的版本</button>
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
