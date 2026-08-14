/**
 * /scoreboard 指令构建器。
 *
 * 语法（1.20.5+ 全版本一致）：
 *   scoreboard objectives add|remove|list|setdisplay|modify …
 *   scoreboard players set|add|remove|get|reset|enable|list|operation …
 *
 * displayName 自 1.20.3 起为文本组件——命令参数里直接写 JSON（如 {"text":"x"}），
 * 这一写法跨 1.20.5+ 所有版本一致，无需版本分支。
 */

import { compact } from "../builder";

export type ScoreboardOperation = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<" | ">" | "><";

export type ScoreboardAction =
  | { kind: "objectives_add"; objective: string; criteria: string; displayName?: string }
  | { kind: "objectives_remove"; objective: string }
  | { kind: "objectives_list" }
  | { kind: "objectives_setdisplay"; slot: string; objective?: string }
  | { kind: "objectives_modify_displayname"; objective: string; displayName: string }
  | { kind: "objectives_modify_rendertype"; objective: string; rendertype: "hearts" | "integer" }
  | { kind: "players_set"; targets: string; objective: string; score: number }
  | { kind: "players_add"; targets: string; objective: string; score: number }
  | { kind: "players_remove"; targets: string; objective: string; score: number }
  | { kind: "players_get"; target: string; objective: string }
  | { kind: "players_reset"; targets: string; objective?: string }
  | { kind: "players_enable"; targets: string; objective: string }
  | { kind: "players_list"; target?: string }
  | {
      kind: "players_operation";
      targets: string;
      objective: string;
      operation: ScoreboardOperation;
      source: string;
      sourceObjective: string;
    };

export interface ScoreboardForm {
  withSlash?: boolean;
  action: ScoreboardAction;
}

/** 把纯文本包成文本组件 JSON（命令参数里直接内联）。 */
function textComponent(text: string): string {
  return compact({ text });
}

export function buildScoreboardCommand(form: ScoreboardForm): string {
  const a = form.action;
  let cmd: string;

  switch (a.kind) {
    case "objectives_add":
      cmd = `scoreboard objectives add ${a.objective} ${a.criteria}`;
      if (a.displayName) cmd += ` ${textComponent(a.displayName)}`;
      break;
    case "objectives_remove":
      cmd = `scoreboard objectives remove ${a.objective}`;
      break;
    case "objectives_list":
      cmd = "scoreboard objectives list";
      break;
    case "objectives_setdisplay":
      cmd = `scoreboard objectives setdisplay ${a.slot}`;
      if (a.objective) cmd += ` ${a.objective}`;
      break;
    case "objectives_modify_displayname":
      cmd = `scoreboard objectives modify ${a.objective} displayname ${textComponent(a.displayName)}`;
      break;
    case "objectives_modify_rendertype":
      cmd = `scoreboard objectives modify ${a.objective} rendertype ${a.rendertype}`;
      break;
    case "players_set":
      cmd = `scoreboard players set ${a.targets} ${a.objective} ${a.score}`;
      break;
    case "players_add":
      cmd = `scoreboard players add ${a.targets} ${a.objective} ${a.score}`;
      break;
    case "players_remove":
      cmd = `scoreboard players remove ${a.targets} ${a.objective} ${a.score}`;
      break;
    case "players_get":
      cmd = `scoreboard players get ${a.target} ${a.objective}`;
      break;
    case "players_reset":
      cmd = `scoreboard players reset ${a.targets}`;
      if (a.objective) cmd += ` ${a.objective}`;
      break;
    case "players_enable":
      cmd = `scoreboard players enable ${a.targets} ${a.objective}`;
      break;
    case "players_list":
      cmd = "scoreboard players list";
      if (a.target) cmd += ` ${a.target}`;
      break;
    case "players_operation":
      cmd = `scoreboard players operation ${a.targets} ${a.objective} ${a.operation} ${a.source} ${a.sourceObjective}`;
      break;
    default: {
      const _exhaustive: never = a;
      throw new Error(`未知 scoreboard 动作: ${JSON.stringify(_exhaustive)}`);
    }
  }

  return form.withSlash ? `/${cmd}` : cmd;
}
