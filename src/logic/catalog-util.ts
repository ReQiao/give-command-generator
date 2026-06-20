/**
 * 目录（中文名 ↔ 资源 id）与键值对（标签 ↔ 值）查询工具，供 UI 与构建器共用。
 */

import type { CatalogRow, PairRow } from "../data/catalog";
import { namespaced } from "./snbt";

/** 把中文名或 id 映射为资源 id；未命中则按命名空间补全后返回。 */
export function mapCatalog(catalog: readonly CatalogRow[], text: string): string {
  for (const row of catalog) {
    if (text === row[0] || text === row[1]) return row[0];
  }
  return namespaced(text);
}

/** 取目录中所有显示名（中文）。 */
export function displayList(catalog: readonly CatalogRow[]): string[] {
  return catalog.map((row) => row[1]);
}

/** 模糊匹配：query 为空则全部命中，否则在整行拼接文本里包含匹配。 */
export function matches(row: readonly unknown[], query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return row.join(" ").toLowerCase().includes(q);
}

/** 标签或值 → 值。 */
export function pairValue(pairs: readonly PairRow[], text: string): string {
  for (const [label, value] of pairs) {
    if (text === label || text === value) return value;
  }
  return text;
}

/** 值或标签 → 标签。 */
export function pairText(pairs: readonly PairRow[], value: string): string {
  for (const [label, itemValue] of pairs) {
    if (value === itemValue || value === label) return label;
  }
  return value;
}
