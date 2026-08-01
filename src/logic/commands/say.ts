/**
 * /say 指令构建器。
 *
 * 语法（全版本一致）：say <message>
 * message 是纯文本，选择器（@a 等）会被展开为玩家名列表。
 */

export interface SayForm {
  withSlash?: boolean;
  message: string;
}

export function buildSayCommand(form: SayForm): string {
  const cmd = `say ${form.message}`;
  return form.withSlash ? `/${cmd}` : cmd;
}
