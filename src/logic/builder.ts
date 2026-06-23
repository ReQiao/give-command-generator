/**
 * 兼容聚合层（barrel）。
 *
 * builder.ts 历史上是单体模块；现已按职责拆分为 types / snbt / catalog-util /
 * color / version / form 与 commands/*。本文件重新导出原有公开 API，保证
 * `./logic/builder` 的既有导入（App.vue、各组件、测试）无需改动。
 *
 * 新代码建议直接从更具体的模块导入；新增指令请放入 commands/ 下并复用 snbt.ts。
 */

// 类型
export type {
  GiveVersion,
  TextComponent,
  RichLine,
  EnchantRow,
  AttributeRow,
  BlockLimitRow,
  EffectItem,
  EffectGroup,
  ToolRuleRow,
  GiveForm,
} from "./types";

// 序列化原语
export {
  compact,
  quote,
  namespaced,
  stripMinecraftNamespace,
  componentId,
  boolByte,
  fmtNumber,
  percentToProbability,
  splitCsv,
} from "./snbt";

// 目录 / 键值对查询
export { mapCatalog, displayList, matches, pairValue, pairText } from "./catalog-util";

// 颜色
export { hexToRgb, rgbToHex, colorLerp, shadowColorInt } from "./color";

// 版本族判定
export { isJava121LegacyFamily, isJava1205Family, isJava1212Family } from "./version";

// 表单
export { createDefaultForm, normalizeForm } from "./form";

// 指令构建器
export { buildGiveCommand } from "./commands/give";
export { buildSayCommand } from "./commands/say";
export type { SayForm } from "./commands/say";
export { buildEffectGiveCommand, buildEffectClearCommand } from "./commands/effect";
export type { EffectGiveForm, EffectClearForm } from "./commands/effect";
export { buildTpCommand } from "./commands/tp";
export type { TpCoordsForm, TpEntityForm } from "./commands/tp";
export { buildSetblockCommand } from "./commands/setblock";
export type { SetblockForm, SetblockMode, ContainerSlot, SetblockCommandBlockOptions } from "./commands/setblock";
export { buildSummonCommand } from "./commands/summon";
export type { SummonForm, SummonPassenger } from "./commands/summon";
export {
  serializeItem,
  serializeContainerItem,
  serializeCustomName,
  serializeAttributes,
  serializeEffects,
  serializeEquipment,
  isModernNbtFamily,
} from "./commands/nbt";
export type { NbtItem, NbtAttribute, NbtEffect, NbtEquipment } from "./commands/nbt";
