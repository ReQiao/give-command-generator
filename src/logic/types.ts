/**
 * 共享类型定义：版本枚举、富文本、各组件行结构与表单。
 *
 * 这些类型不依赖任何运行时代码，可被所有指令构建器与 UI 组件复用。
 */

export type GiveVersion =
  | "java_1_20_5"
  | "java_1_21"
  | "java_1_21_1"
  | "java_1_21_2"
  | "java_1_21_3"
  | "java_1_21_4"
  | "java_1_21_5"
  | "java_1_21_6"
  | "java_1_21_9"
  | "java_1_21_11_plus"
  | "java_26_1"
  | "java_26_2_plus"
  | "bedrock";

export interface TextComponent {
  text: string;
  bold?: boolean;
  italic?: boolean;
  underlined?: boolean;
  strikethrough?: boolean;
  color?: string;
  shadow_color?: number;
}

export type RichLine = TextComponent[];

export interface EnchantRow {
  id: string;
  level: number | string;
}

export interface AttributeRow {
  type: string;
  amount: number | string;
  slot: string;
  operation: string;
  id: string;
}

export interface BlockLimitRow {
  block: string;
  type: string;
}

export interface EffectItem {
  id: string;
  duration?: number | string;
  amplifier?: number | string;
  show_particles?: boolean;
  show_icon?: boolean;
}

export interface EffectGroup {
  type: "apply_effects" | "remove_effects" | "clear_all_effects" | "teleport_randomly";
  probability_percent?: number | string;
  diameter?: number | string;
  effects?: Array<EffectItem | string>;
}

export interface ToolRuleRow {
  blocks: string[] | string;
  speed: number | string;
  correct_for_drops: string;
}

export interface GiveForm {
  version: GiveVersion;
  target: string;
  item: string;
  itemSearch: string;
  count: number;
  withSlash: boolean;
  templateName: string;
  bedrockDataValue: number;
  bedrockItemLock: string;
  bedrockKeepOnDeath: boolean;
  displayName: RichLine[];
  itemName: RichLine[];
  lore: RichLine[];
  rarity: string;
  glint: string;
  enchantments: EnchantRow[];
  attributes: AttributeRow[];
  blockLimits: BlockLimitRow[];
  unbreakable: boolean;
  glider: boolean;
  deathProtection: boolean;
  deathEffects: EffectGroup[];
  damageEnabled: boolean;
  damage: number;
  maxDamageEnabled: boolean;
  maxDamage: number;
  stackEnabled: boolean;
  maxStackSize: number;
  repairEnabled: boolean;
  repairCost: number;
  hiddenComponents: string;
  foodEnabled: boolean;
  nutrition: number;
  saturation: number;
  alwaysEat: string;
  consumableEnabled: boolean;
  consumeSeconds: number;
  consumeSound: string;
  consumeParticles: string;
  consumeEffects: EffectGroup[];
  toolEnabled: boolean;
  defaultMiningSpeed: number;
  damagePerBlock: number;
  toolRules: ToolRuleRow[];
}
