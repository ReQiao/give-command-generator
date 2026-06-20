/**
 * SNBT / 组件值序列化原语（共享序列化器）。
 *
 * 这是跨指令复用的核心：give 的 item 组件值、以及未来 setblock/summon 的方块实体
 * 与实体 NBT，都依赖这里的字符串化、命名空间处理与数值格式化。
 */

/** 紧凑 JSON 序列化（用于现代版本直接内联 JSON 文本组件 / 基岩版组件对象）。 */
export function compact(value: unknown): string {
  return JSON.stringify(value);
}

/** 把任意 JSON 值包成 SNBT 单引号字符串（旧版本文本组件写法）。 */
export function snbtJsonString(value: unknown): string {
  return `'${JSON.stringify(value).replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}

/** 标准 JSON 双引号字符串。 */
export function quote(value: string): string {
  return JSON.stringify(value);
}

/** 补全命名空间（默认 minecraft:），已含冒号则原样返回。 */
export function namespaced(value: string, namespace = "minecraft"): string {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return text.includes(":") ? text : `${namespace}:${text}`;
}

/** 去掉 minecraft: 前缀。 */
export function stripMinecraftNamespace(value: string): string {
  const text = String(value ?? "").trim();
  return text.startsWith("minecraft:") ? text.slice("minecraft:".length) : text;
}

/** 归一化为去命名空间的组件/资源 id（先补全再剥离，统一格式）。 */
export function componentId(value: string): string {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return stripMinecraftNamespace(namespaced(text));
}

export function boolByte(value: boolean): string {
  return value ? "1b" : "0b";
}

/** 数字格式化：整数原样，浮点去掉尾随 0。 */
export function fmtNumber(value: unknown): string {
  const num = Number(value);
  if (!Number.isFinite(num)) return String(value ?? "").trim();
  if (Number.isInteger(num)) return String(num);
  return num.toFixed(10).replace(/0+$/, "").replace(/\.$/, "");
}

/** 百分比（0-100）转概率（0-1）字符串。 */
export function percentToProbability(value: unknown): string {
  const num = Math.max(0, Math.min(100, Number(value) || 0));
  return fmtNumber(num / 100);
}

/** 拆分逗号分隔字符串（或直接传数组），去空白与空项。 */
export function splitCsv(value: string | string[]): string[] {
  if (Array.isArray(value)) return value.map((item) => item.trim()).filter(Boolean);
  return String(value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}
