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
import {
  ATTRIBUTES,
  BEDROCK_BLOCKS,
  BEDROCK_ENTITIES,
  BEDROCK_ITEMS,
  BLOCKS,
  EFFECTS,
  ENCHANTS,
  ENTITIES,
  ITEMS,
  PARTICLES,
  type CatalogRow,
} from "../data/catalog";
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
/** 把目录数组变成 (行数组, id 集合) 一对，避免每次校验都重新遍历。 */
function indexed(catalog: readonly CatalogRow[]) {
  return { rows: catalog, ids: new Set(catalog.map((row) => row[0])) };
}

/**
 * 按版本挑目录：基岩版和 Java 是两套 ID 体系，同一个东西名字经常不同
 * （蜘蛛网 cobweb/web），拿错表会同时犯两个方向的错——真实的基岩 id 被当成
 * "AI 编造"拦掉，而 Java id 反倒放行、拼出一条基岩里不存在的指令。
 *
 * 附魔/药水效果/属性暂时两版共用 Java 表：我们只生成了基岩的物品/方块/实体
 * 三张表，而且基岩版的 give 构建器本来就不输出附魔和效果（见 buildBedrock），
 * 目前碰不到这个差异。真要支持基岩的 enchant/effect 指令时，得先把
 * scripts/bedrock-id/ 里的 enchantType.json、effect.json 也生成出来。
 */
const JAVA_CATALOGS = {
  items: indexed(ITEMS),
  blocks: indexed(BLOCKS),
  entities: indexed(ENTITIES),
};
const BEDROCK_CATALOGS = {
  items: indexed(BEDROCK_ITEMS),
  blocks: indexed(BEDROCK_BLOCKS),
  entities: indexed(BEDROCK_ENTITIES),
};
// 两版共用（原因见上）
const ENCHANT_CAT = indexed(ENCHANTS);
const EFFECT_CAT = indexed(EFFECTS);
const ATTRIBUTE_CAT = indexed(ATTRIBUTES);
const PARTICLE_CAT = indexed(PARTICLES);

type VersionCatalogs = typeof JAVA_CATALOGS;
const catalogsFor = (version: GiveVersion): VersionCatalogs =>
  version === "bedrock" ? BEDROCK_CATALOGS : JAVA_CATALOGS;

type Indexed = { rows: readonly CatalogRow[]; ids: Set<string> };

/** 取粒子 id 里花括号之前的部分（minecraft:dust{color:[...]} -> minecraft:dust）。 */
function particleIdOnly(raw: unknown): unknown {
  if (typeof raw !== "string") return raw;
  const brace = raw.indexOf("{");
  return brace === -1 ? raw : raw.slice(0, brace).trim();
}

/** catalog 里没有的一律视为 AI 编造；命中就返回 null。 */
function catalogMiss(kind: string, raw: unknown, cat: Indexed): string | null {
  if (typeof raw !== "string" || !raw.trim()) return null; // 空字段留给各自的必填校验去报错，这里不重复报
  if (cat.ids.has(mapCatalog(cat.rows, raw))) return null;
  return `${kind} "${raw}" 不在官方目录里，疑似 AI 编造，已拦截`;
}

/** 校验一组 { id } 形状的附魔/效果条目（give.enchantments、summon.effects 等）。 */
function firstCatalogMissInList(kind: string, list: unknown, cat: Indexed): string | null {
  if (!Array.isArray(list)) return null;
  for (const row of list) {
    const err = catalogMiss(kind, (row as { id?: unknown })?.id, cat);
    if (err) return err;
  }
  return null;
}

/** 校验 summon.equipment：{ mainhand?: { id, enchantments? }, head?: ..., ... }。 */
function equipmentCatalogMiss(equipment: unknown, cats: VersionCatalogs): string | null {
  if (!equipment || typeof equipment !== "object") return null;
  for (const slot of Object.values(equipment as Record<string, unknown>)) {
    if (!slot || typeof slot !== "object") continue;
    const slotObj = slot as Record<string, unknown>;
    const err =
      catalogMiss("物品", slotObj.id, cats.items) ??
      firstCatalogMissInList("附魔", slotObj.enchantments, ENCHANT_CAT);
    if (err) return err;
  }
  return null;
}

/** 校验 summon.passengers[]：每个乘客也是一个实体，同样不能是编出来的。 */
function passengersCatalogMiss(passengers: unknown, cats: VersionCatalogs): string | null {
  if (!Array.isArray(passengers)) return null;
  for (const p of passengers) {
    if (!p || typeof p !== "object") continue;
    const err = catalogMiss("乘客实体类型", (p as Record<string, unknown>).entityType, cats.entities);
    if (err) return err;
  }
  return null;
}

/** 校验 setblock.containerItems[]：{ slot, item: { id, enchantments? } }。 */
function containerItemsCatalogMiss(items: unknown, cats: VersionCatalogs): string | null {
  if (!Array.isArray(items)) return null;
  for (const entry of items) {
    const item = (entry as { item?: unknown })?.item;
    if (!item || typeof item !== "object") continue;
    const itemObj = item as Record<string, unknown>;
    const err =
      catalogMiss("容器内物品", itemObj.id, cats.items) ??
      firstCatalogMissInList("附魔", itemObj.enchantments, ENCHANT_CAT);
    if (err) return err;
  }
  return null;
}

/** 校验 fill.replaceFilter / clone.filter 这种 { block, blockstate? } 过滤器。 */
function blockFilterCatalogMiss(kind: string, filter: unknown, cats: VersionCatalogs): string | null {
  if (!filter || typeof filter !== "object") return null;
  return catalogMiss(kind, (filter as Record<string, unknown>).block, cats.blocks);
}

/**
 * 校验 scoreboard 判据里内嵌的物品 id。
 *
 * 提示词主动教了 minecraft.used:minecraft.<item> / minecraft.custom:minecraft.<stat>
 * 这类统计判据（见 prompt.ts 的"特殊计分板判据"一节），冒号后面那截是真实的物品 id，
 * 编错了整个计分板就永远不会涨分，而且失败得很安静——不报错，只是没反应。
 * 只挑 used/mined/crafted/broken/picked_up/dropped/killed 这几类"后面接物品 id"的
 * 判据来查；custom 后面接的是统计项名（sneak_time、jump 之类），不在物品表里，跳过。
 */
const ITEM_BACKED_CRITERIA = /^minecraft\.(used|mined|crafted|broken|picked_up|dropped)/;
function criteriaCatalogMiss(criteria: unknown, cats: VersionCatalogs): string | null {
  if (typeof criteria !== "string" || !criteria.includes(":")) return null;
  const [head, tail] = [criteria.slice(0, criteria.indexOf(":")), criteria.slice(criteria.indexOf(":") + 1)];
  if (!ITEM_BACKED_CRITERIA.test(head)) return null;
  // 判据里用点号分隔命名空间（minecraft.stone），转成正常 id 再查
  const asId = tail.replace(".", ":");
  return catalogMiss("计分板判据里的物品", asId, cats.items);
}

/** 按意图类型校验涉及官方目录的字段。返回非 null 即视为构建失败。 */
function validateIntentCatalog(intent: CommandIntent, version: GiveVersion): string | null {
  const form = intent.form as Record<string, unknown>;
  const cats = catalogsFor(version);
  switch (intent.command) {
    case "give":
      return (
        catalogMiss("物品", form.item, cats.items) ??
        firstCatalogMissInList("附魔", form.enchantments, ENCHANT_CAT)
      );
    case "setblock":
      return (
        catalogMiss("方块", form.block, cats.blocks) ??
        containerItemsCatalogMiss(form.containerItems, cats)
      );
    case "fill":
      return (
        catalogMiss("方块", form.block, cats.blocks) ??
        blockFilterCatalogMiss("替换过滤方块", form.replaceFilter, cats)
      );
    case "clone":
      return blockFilterCatalogMiss("克隆过滤方块", form.filter, cats);
    case "enchant":
      return catalogMiss("附魔", form.enchantment, ENCHANT_CAT);
    case "effect_give":
      return catalogMiss("药水效果", form.effect, EFFECT_CAT);
    case "effect_clear":
      // effect_give 一直有校验，effect_clear 之前漏了，两者不该不一致
      return catalogMiss("药水效果", form.effect, EFFECT_CAT);
    case "attribute":
      return catalogMiss("属性", form.attribute, ATTRIBUTE_CAT);
    case "scoreboard":
      return criteriaCatalogMiss((form.action as Record<string, unknown> | undefined)?.criteria, cats);
    case "summon":
      return (
        catalogMiss("实体类型", form.entityType, cats.entities) ??
        firstCatalogMissInList("药水效果", form.effects, EFFECT_CAT) ??
        equipmentCatalogMiss(form.equipment, cats) ??
        passengersCatalogMiss(form.passengers, cats)
      );
    case "particle":
      // 参数化粒子（minecraft:dust{color:[...]}）的花括号部分不参与查表，
      // 否则整类带附加数据的粒子都会被误判成编造的（同 particle.ts 的切法）。
      return catalogMiss("粒子", particleIdOnly(form.name), PARTICLE_CAT);
    default:
      return null;
  }
}

/** 把单条意图分派到对应构建器。version 为目标 Minecraft 版本。 */
export function dispatchIntent(intent: CommandIntent, version: GiveVersion): DispatchResult {
  const catalogError = validateIntentCatalog(intent, version);
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
