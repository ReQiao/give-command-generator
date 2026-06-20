/**
 * 版本族判定与组件能力档案（profile）。
 *
 * give 的现代族（1.20.5 / 1.21.2 / 1.21.5+）共用同一套构建逻辑，差异通过
 * profile 的布尔开关表达；1.21 / 1.21.1 与基岩版各自单独处理。
 */

import type { GiveVersion } from "./types";

/** 现代族组件能力档案：控制文本写法、谓词包裹与各组件是否受支持。 */
export interface ModernProfile {
  textAsSnbtString: boolean;
  adventurePredicateWrapper: boolean;
  supportsTooltipDisplay: boolean;
  supportsConsumable: boolean;
  supportsGlider: boolean;
  supportsDeathProtection: boolean;
  supportsAttributeModifiers: boolean;
}

/** 1.21.5+（含 26.x）：JSON 文本、直接谓词列表、全组件支持。 */
export const MODERN_PROFILE: ModernProfile = {
  textAsSnbtString: false,
  adventurePredicateWrapper: false,
  supportsTooltipDisplay: true,
  supportsConsumable: true,
  supportsGlider: true,
  supportsDeathProtection: true,
  supportsAttributeModifiers: true,
};

/** 1.21.2 / 1.21.3 / 1.21.4：SNBT 文本、谓词包裹，无 tooltip_display。 */
export const JAVA_1_21_2_PROFILE: ModernProfile = {
  textAsSnbtString: true,
  adventurePredicateWrapper: true,
  supportsTooltipDisplay: false,
  supportsConsumable: true,
  supportsGlider: true,
  supportsDeathProtection: true,
  supportsAttributeModifiers: true,
};

/** 1.20.5 / 1.20.6：最早的组件支持，无 consumable/glider/death_protection/attribute_modifiers。 */
export const JAVA_1_20_5_PROFILE: ModernProfile = {
  textAsSnbtString: true,
  adventurePredicateWrapper: true,
  supportsTooltipDisplay: false,
  supportsConsumable: false,
  supportsGlider: false,
  supportsDeathProtection: false,
  supportsAttributeModifiers: false,
};

export function isJava121LegacyFamily(version: GiveVersion): boolean {
  return version === "java_1_21" || version === "java_1_21_1";
}

export function isJava1205Family(version: GiveVersion): boolean {
  return version === "java_1_20_5";
}

export function isJava1212Family(version: GiveVersion): boolean {
  return version === "java_1_21_2" || version === "java_1_21_3" || version === "java_1_21_4";
}

export function normalizeVersion(value: unknown): GiveVersion {
  const text = String(value ?? "").trim();
  if (
    text === "java_1_20_5" ||
    text === "java_1_21" ||
    text === "java_1_21_1" ||
    text === "java_1_21_2" ||
    text === "java_1_21_3" ||
    text === "java_1_21_4" ||
    text === "java_1_21_5" ||
    text === "java_1_21_6" ||
    text === "java_1_21_9" ||
    text === "java_1_21_11_plus" ||
    text === "java_26_1" ||
    text === "java_26_2_plus" ||
    text === "bedrock"
  ) {
    return text;
  }
  return "java_1_21_11_plus";
}
