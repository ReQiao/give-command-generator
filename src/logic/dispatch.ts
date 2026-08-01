/**
 * 指令分派器（AI 意图 → 确定性命令字符串）。
 *
 * AI 只负责把自然语言翻译成「指令意图」（CommandIntent）——描述“做什么”，
 * 不负责拼写 1.20.5+ 的精确组件/NBT 语法（AI 在这上面极易出错）。
 * 真正的语法生成交给 commands/* 下经 mc-verifier 实证过的确定性构建器。
 *
 * 这样 AI 幻觉只会影响“意图”，不会产出语法非法的命令——非法意图在此被捕获并报错。
 */

import { buildGiveCommand, normalizeForm, type GiveVersion } from "./builder";
import { buildSayCommand, type SayForm } from "./commands/say";
import {
  buildEffectClearCommand,
  buildEffectGiveCommand,
  type EffectClearForm,
  type EffectGiveForm,
} from "./commands/effect";
import { buildTpCommand, type TpCoordsForm, type TpEntityForm } from "./commands/tp";
import { buildSetblockCommand, type SetblockForm } from "./commands/setblock";
import { buildSummonCommand, type SummonForm } from "./commands/summon";
import { buildFillCommand, type FillForm } from "./commands/fill";
import { buildCloneCommand, type CloneForm } from "./commands/clone";
import { buildEnchantCommand, type EnchantForm } from "./commands/enchant";
import { buildExecuteCommand, type ExecuteForm } from "./commands/execute";
import { buildScoreboardCommand, type ScoreboardForm } from "./commands/scoreboard";
import { buildAttributeCommand, type AttributeForm } from "./commands/attribute";
import { buildParticleCommand, type ParticleForm } from "./commands/particle";

/** 版本由分派器统一注入，AI 不需要（也不应该）自己填。 */
type Versionless<T> = Omit<T, "version"> & { version?: GiveVersion };

/** AI 产出的单条指令意图。`form` 为对应构建器的（可能不完整的）表单数据。 */
export type CommandIntent =
  | { command: "give"; form: Record<string, unknown> }
  | { command: "say"; form: SayForm }
  | { command: "effect_give"; form: EffectGiveForm }
  | { command: "effect_clear"; form: EffectClearForm }
  | { command: "tp"; form: TpCoordsForm | TpEntityForm }
  | { command: "setblock"; form: Versionless<SetblockForm> }
  | { command: "summon"; form: Versionless<SummonForm> }
  | { command: "fill"; form: FillForm }
  | { command: "clone"; form: CloneForm }
  | { command: "enchant"; form: EnchantForm }
  | { command: "execute"; form: ExecuteForm }
  | { command: "scoreboard"; form: ScoreboardForm }
  | { command: "attribute"; form: Versionless<AttributeForm> }
  | { command: "particle"; form: ParticleForm };

export interface DispatchResult {
  /** 原始意图（便于 UI 回显 / 调试）。 */
  intent: CommandIntent;
  /** 生成的命令字符串；失败时为 null。 */
  command: string | null;
  /** 失败原因；成功时为 null。 */
  error: string | null;
  /**
   * 是否需要每 tick 持续执行（目前只有 execute 意图能标记 form.loop=true）。
   * UI 据此区分「一次性指令，可直接复制」和「循环侦测，需要部署成 datapack」。
   */
  loop: boolean;
}

/** 把单条意图分派到对应构建器。version 为目标 Minecraft 版本。 */
export function dispatchIntent(intent: CommandIntent, version: GiveVersion): DispatchResult {
  try {
    let command: string;
    switch (intent.command) {
      case "give":
        // normalizeForm 会把脏数据 / 缺字段补全为合法 GiveForm
        command = buildGiveCommand(normalizeForm({ ...intent.form, version }));
        break;
      case "say":
        command = buildSayCommand(intent.form);
        break;
      case "effect_give":
        command = buildEffectGiveCommand(intent.form);
        break;
      case "effect_clear":
        command = buildEffectClearCommand(intent.form);
        break;
      case "tp":
        command = buildTpCommand(intent.form);
        break;
      case "setblock":
        command = buildSetblockCommand({ ...intent.form, version });
        break;
      case "summon":
        command = buildSummonCommand({ ...intent.form, version });
        break;
      case "fill":
        command = buildFillCommand(intent.form);
        break;
      case "clone":
        command = buildCloneCommand(intent.form);
        break;
      case "enchant":
        command = buildEnchantCommand(intent.form);
        break;
      case "execute":
        command = buildExecuteCommand(intent.form);
        break;
      case "scoreboard":
        command = buildScoreboardCommand(intent.form);
        break;
      case "attribute":
        command = buildAttributeCommand({ ...intent.form, version });
        break;
      case "particle":
        command = buildParticleCommand(intent.form);
        break;
      default: {
        const _exhaustive: never = intent;
        const badCommand = (_exhaustive as { command?: unknown })?.command;
        return { intent, command: null, error: `未知指令类型: ${JSON.stringify(badCommand)}`, loop: false };
      }
    }
    const loop = intent.command === "execute" && intent.form.loop === true;
    return { intent, command, error: null, loop };
  } catch (err) {
    return { intent, command: null, error: err instanceof Error ? err.message : String(err), loop: false };
  }
}

/** 批量分派。返回与输入顺序一致的结果数组。 */
export function dispatchIntents(intents: CommandIntent[], version: GiveVersion): DispatchResult[] {
  return intents.map((intent) => dispatchIntent(intent, version));
}
