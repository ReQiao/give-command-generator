/**
 * 指令分派器（AI 意图 → 确定性命令字符串）。
 *
 * AI 只负责把自然语言翻译成「指令意图」（CommandIntent）——描述“做什么”，
 * 不负责拼写 1.20.5+ 的精确组件/NBT 语法（AI 在这上面极易出错）。
 * 真正的语法生成交给 commands/* 下经 mc-verifier 实证过的确定性构建器。
 *
 * 这样 AI 幻觉只会影响“意图”，不会产出语法非法的命令——非法意图在此被捕获并报错。
 */

import { buildGiveCommand, mapCatalog, normalizeForm, type GiveVersion } from "./builder";
import { ATTRIBUTES, BLOCKS, EFFECTS, ENCHANTS, ENTITIES, ITEMS, type CatalogRow } from "../data/catalog";
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

/**
 * AI 幻觉防线之二：目录存在性校验。
 *
 * mapCatalog 对手动模式很宽容——匹配不上就假设是模组物品/自定义 id，直接
 * namespaced() 放行，这是刻意的（手动模式的用户可能真的要填模组内容）。
 * 但 AI 生成的内容不该有这种自由度：系统提示词已经把官方目录喂给它了，
 * 匹配不上目录，几乎总是编造，必须在这里拦下来，而不是让它悄悄拼进最终命令。
 * 只有本文件（AI 面板专用的 dispatchIntent/dispatchIntents）会走这层校验，
 * 手动模式直接调 builder，不受影响。
 */
const ITEM_IDS = new Set(ITEMS.map((row) => row[0]));
const BLOCK_IDS = new Set(BLOCKS.map((row) => row[0]));
const ENCHANT_IDS = new Set(ENCHANTS.map((row) => row[0]));
const EFFECT_IDS = new Set(EFFECTS.map((row) => row[0]));
const ATTRIBUTE_IDS = new Set(ATTRIBUTES.map((row) => row[0]));
const ENTITY_IDS = new Set(ENTITIES.map((row) => row[0]));

/** catalog 里没有的一律视为 AI 编造；命中就返回 null。 */
function catalogMiss(kind: string, raw: unknown, catalog: readonly CatalogRow[], ids: Set<string>): string | null {
  if (typeof raw !== "string" || !raw.trim()) return null; // 空字段留给各自的必填校验去报错，这里不重复报
  const resolved = mapCatalog(catalog, raw);
  if (ids.has(resolved)) return null;
  return `${kind} "${raw}" 不在官方目录里，疑似 AI 编造，已拦截`;
}

/** 校验一组 { id } 形状的附魔/效果条目（give.enchantments、summon.effects 等）。 */
function firstCatalogMissInList(
  kind: string,
  list: unknown,
  catalog: readonly CatalogRow[],
  ids: Set<string>,
): string | null {
  if (!Array.isArray(list)) return null;
  for (const row of list) {
    const err = catalogMiss(kind, (row as { id?: unknown })?.id, catalog, ids);
    if (err) return err;
  }
  return null;
}

/** 校验 summon.equipment：{ mainhand?: { id, enchantments? }, head?: ..., ... }。 */
function equipmentCatalogMiss(equipment: unknown): string | null {
  if (!equipment || typeof equipment !== "object") return null;
  for (const slot of Object.values(equipment as Record<string, unknown>)) {
    if (!slot || typeof slot !== "object") continue;
    const slotObj = slot as Record<string, unknown>;
    const err = catalogMiss("物品", slotObj.id, ITEMS, ITEM_IDS) ?? firstCatalogMissInList("附魔", slotObj.enchantments, ENCHANTS, ENCHANT_IDS);
    if (err) return err;
  }
  return null;
}

/** 按意图类型校验涉及官方目录的字段。返回非 null 即视为构建失败。 */
function validateIntentCatalog(intent: CommandIntent): string | null {
  const form = intent.form as Record<string, unknown>;
  switch (intent.command) {
    case "give":
      return (
        catalogMiss("物品", form.item, ITEMS, ITEM_IDS) ??
        firstCatalogMissInList("附魔", form.enchantments, ENCHANTS, ENCHANT_IDS)
      );
    case "setblock":
    case "fill":
      return catalogMiss("方块", form.block, BLOCKS, BLOCK_IDS);
    case "enchant":
      return catalogMiss("附魔", form.enchantment, ENCHANTS, ENCHANT_IDS);
    case "effect_give":
      return catalogMiss("药水效果", form.effect, EFFECTS, EFFECT_IDS);
    case "attribute":
      return catalogMiss("属性", form.attribute, ATTRIBUTES, ATTRIBUTE_IDS);
    case "summon":
      return (
        catalogMiss("实体类型", form.entityType, ENTITIES, ENTITY_IDS) ??
        firstCatalogMissInList("药水效果", form.effects, EFFECTS, EFFECT_IDS) ??
        equipmentCatalogMiss(form.equipment)
      );
    default:
      return null;
  }
}

/** 把单条意图分派到对应构建器。version 为目标 Minecraft 版本。 */
export function dispatchIntent(intent: CommandIntent, version: GiveVersion): DispatchResult {
  const catalogError = validateIntentCatalog(intent);
  if (catalogError) {
    return { intent, command: null, error: catalogError, loop: false };
  }
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
