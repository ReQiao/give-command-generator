/**
 * 颜色工具：十六进制 ↔ RGB、渐变插值、阴影色编码。供富文本编辑器使用。
 */

export function hexToRgb(value: string): [number, number, number] {
  const text = value.trim();
  if (!/^#[0-9a-fA-F]{6}$/.test(text)) throw new Error("颜色必须是 #RRGGBB");
  return [
    Number.parseInt(text.slice(1, 3), 16),
    Number.parseInt(text.slice(3, 5), 16),
    Number.parseInt(text.slice(5, 7), 16),
  ];
}

export function rgbToHex(value: [number, number, number]): string {
  return `#${value.map((item) => item.toString(16).padStart(2, "0")).join("")}`;
}

/** 在 start..end 之间生成 count 个等距渐变色。 */
export function colorLerp(start: string, end: string, count: number): string[] {
  if (count <= 0) return [];
  const a = hexToRgb(start);
  const b = hexToRgb(end);
  if (count === 1) return [rgbToHex(a)];
  return Array.from({ length: count }, (_, index) => {
    const ratio = index / (count - 1);
    return rgbToHex([
      Math.round(a[0] + (b[0] - a[0]) * ratio),
      Math.round(a[1] + (b[1] - a[1]) * ratio),
      Math.round(a[2] + (b[2] - a[2]) * ratio),
    ]);
  });
}

/** 把十六进制颜色 + 透明度百分比编码为带符号 32 位整数（文本阴影色）。 */
export function shadowColorInt(hexColor: string, alphaPercent: number): number {
  const [r, g, b] = hexToRgb(hexColor);
  const alpha = Math.round(Math.max(0, Math.min(100, alphaPercent)) / 100 * 255);
  const value = (alpha << 24) | (r << 16) | (g << 8) | b;
  return value >= 2 ** 31 ? value - 2 ** 32 : value;
}
