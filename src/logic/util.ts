/**
 * 通用数值/目标归一化与 id 生成工具，供表单与各指令构建器共用。
 */

export function normalizeInt(value: unknown, fallback: number, min: number): number {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(min, Math.floor(num));
}

export function normalizeNumber(value: unknown, fallback: number, min: number): number {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(min, num);
}

export function normalizeTarget(value: string): string {
  const text = String(value ?? "").trim();
  return text.length > 0 ? text : "@a";
}

export function cryptoId(): string {
  if ("crypto" in globalThis && "randomUUID" in crypto) return crypto.randomUUID();
  return String(Date.now());
}
