# Give 指令生成器项目更新对话总结

这份总结用于开启新对话时快速接续项目上下文。当前项目已经不是 Python + PySide6，而是 **Tauri v2 + Vue 3 + TypeScript** 桌面应用。

## 1. 用户与协作偏好

- 用户主要使用中文交流，希望回答直接、可操作，不要凭空猜测。
- 遇到报错时要结合具体输出分析，能本地验证就先验证。
- 代码相关任务默认可以直接修改仓库，不要只给方案。
- 不要误把旧 PySide6 状态当成当前状态。当前仓库是 Tauri + Vue + TypeScript。
- 视觉和交互改动要尽量贴近原 Python 版本，不要随意改变功能和布局含义；用户特别强调“动画除外，不要有任何改动”。
- 用户很在意界面统一性：深蓝玻璃风、弹窗/菜单/提示/输入框都要统一风格，不能出现系统默认白灰控件。

## 2. 当前仓库位置与技术栈

项目路径：

```text
C:\Users\Aga Zuoshi\Desktop\Give-command-generator
```

当前技术栈：

```text
Tauri v2
Vue 3
TypeScript
Vite
Rust / Cargo
CSS
```

当前开发运行命令：

```powershell
npm install
npm run tauri dev
```

当前构建命令：

```powershell
npm run tauri build
```

Windows 构建产物通常在：

```text
src-tauri/target/release/
src-tauri/target/release/bundle/
src-tauri/target/release/bundle/nsis/
```

直接运行的 exe 通常在：

```text
src-tauri/target/release/give-command-generator.exe
```

## 3. 项目目标

这是一个面向 Minecraft 的 `/give` 指令生成器，目标是解决 MC 新手和指令熟手编辑 `/give` JSON、物品组件时步骤繁琐、容易写错的问题。

当前 README 描述方向应保持为：

```text
一个基于 Tauri v2 + Vue 3 + TypeScript 的桌面应用。
中文深蓝玻璃风 UI。
支持富文本、渐变颜色、阴影颜色、模板导入导出、自动保存、多版本指令生成。
当前支持 Java 1.21.11+ 与 Bedrock 两种模式。
```

不要再写成 Python + PySide6 项目。

## 4. 旧 PySide6 背景

项目最初是 Python + PySide6 桌面程序，大致结构曾经包括：

```text
give_generator/
  app.py
  builder.py
  config.py
  data.py
  main_window.py
  style.py
  utils.py
  widgets/
    color_selector.py
    common.py
    effect_editor.py
    modal.py
    rich_text.py
```

旧版功能包括：

- Java 1.21.11+ `/give` 组件语法
- Bedrock `/give` 基础语法
- custom_name / item_name / lore
- 富文本颜色、渐变、阴影颜色
- enchantments
- attribute_modifiers
- can_place_on / can_break
- unbreakable / glider
- death_protection / death_effects
- damage / max_damage / max_stack_size / repair_cost
- tooltip_display
- food
- consumable / on_consume_effects
- tool.rules
- 模板保存与读取
- 自动保存
- 复制指令
- Java / Bedrock 模式切换

旧 PySide6 的 Tab 页面动画曾经很不稳定，出现过残影、白屏、字体警告等问题，所以后来决定迁移到 Tauri + Vue，用 CSS/Vue transition 实现更稳定的动画。

## 5. 关键 Minecraft 生成规则

迁移和后续修改时要继续遵守这些规则：

```text
Java 1.21.11+ 附魔 ID 输出时去掉 minecraft:
Java 1.21.11+ 属性 type 输出时去掉 minecraft:
attribute_modifiers 不要输出 slot:any，默认视为全槽位
glider={} 要保留，不能因为空对象被删掉
Bedrock 物品 ID 和方块 ID 默认去掉 minecraft:
tool.rules 支持 speed 和 correct_for_drops
consumable.on_consume_effects 支持多种效果和概率
death_protection.death_effects 与食用效果类似
tooltip_display 某些默认 true 的字段默认不输出，改动后才输出
```

## 6. 当前主要源码文件

当前核心文件：

```text
src/App.vue
src/style.css
src/logic/builder.ts
src/data/catalog.ts
```

当前组件：

```text
src/components/RichTextEditor.vue
src/components/EffectEditor.vue
src/components/CustomSelect.vue
src/components/CatalogCombo.vue
src/components/InfoTip.vue
src/components/NumberInput.vue
```

各文件职责：

- `App.vue`：主界面、表单状态、模板导入导出、内置模板、生成/复制、Tabs、整体布局。
- `style.css`：深蓝玻璃风 UI、按钮、表单、菜单、提示、动画、响应式布局。
- `builder.ts`：Java / Bedrock 指令生成逻辑。
- `catalog.ts`：物品、方块、附魔、属性等数据。
- `RichTextEditor.vue`：富文本、渐变、颜色、阴影等编辑。
- `EffectEditor.vue`：食用效果、死亡效果等编辑。
- `CustomSelect.vue`：替代原生 select 的统一风格下拉菜单。
- `CatalogCombo.vue`：替代 datalist 的搜索补全组件。
- `InfoTip.vue`：统一风格的悬浮解释提示。
- `NumberInput.vue`：替代原生 number input 的深蓝玻璃风数字输入和加减按钮。

## 7. 已完成的重要迁移与改动

### Tauri / Vue 基础迁移

- 用 Tauri v2 + Vue 3 + TypeScript 替代 PySide6。
- 重写默认 Tauri 模板，做成 Give 指令生成器。
- 新增 `src/logic/builder.ts`、`src/style.css`，重写 `src/App.vue`。
- 将旧 Python 数据迁移到 `src/data/catalog.ts`。
- 加入 `RichTextEditor.vue` 和 `EffectEditor.vue`。
- `src-tauri/tauri.conf.json` 的窗口标题和尺寸改成适合应用的桌面窗口。

### README 更新

README 已从 PySide6 描述改为 Tauri v2 + Vue 3 + TypeScript 描述。

当前 README 应说明：

- 当前支持 Java 1.21.11+ / Bedrock。
- 当前保存草稿和动画设置使用 `localStorage`。
- 模板通过 JSON 文件导入导出。
- 内置模板来自根目录 `templates/*.json`。
- 构建命令是 `npm run tauri build`。

### 图标

源图标：

```text
minecraft_give_generator_icon_1024.png
```

已经用 Tauri 图标命令生成过多尺寸图标：

```powershell
npm run tauri icon minecraft_give_generator_icon_1024.png
```

生成位置：

```text
src-tauri/icons/
```

`tauri.conf.json` 已验证引用这些图标。用户说 build 自己来，图标任务只需要改好配置和资源。

### Git / GitHub

用户曾经误在 `src-tauri` 里面初始化 Git，后来确认应该删除 `src-tauri/.git`，只保留项目根目录的 `.git`。

如果 `git add .` 很慢，通常是因为：

- `node_modules`
- `dist`
- `src-tauri/target`
- 构建产物或缓存

根目录 `.gitignore` 应忽略这些内容。不要把 `src-tauri/target`、`node_modules`、`dist` 提交进仓库。

## 8. 当前 UI 和交互状态

近期重点修复了用户指出的这些问题：

- 生成指令后有文本框显示结果。
- “版本”这类带 `+` 的菜单不再使用白色透明原生下拉。
- 全部原生 `select` / `datalist` 已替换为统一深蓝玻璃风组件。
- Tab 补全增强：
  - 输入中文可补全，例如 `耐`。
  - 输入英文 ID 可补全，例如 `unb`。
  - 输入完整命名空间 ID 可补全，例如 `minecraft:unb`。
  - 菜单中统一显示中文名，例如“耐久”。
- 左侧物品选择菜单不再超出屏幕，改为可滚动、随分辨率伸缩。
- 内置模板菜单加入，可直接选择 `templates` 文件夹里的模板。
- 鼠标悬浮 `?` 可以显示易懂解释。
- 数字输入框不再使用系统原生白灰箭头，改成自定义 `NumberInput.vue`。
- 复选框 `□` 风格已改得更贴近深蓝玻璃风。
- Tooltip、下拉菜单、补全菜单已使用 Teleport 到 `body`，避免被右侧主框或容器边界裁切。
- 左侧所有 `?` 的提示会根据位置判断向右或向左显示，避免超出窗口。

最新用户关注点是 tooltip 位置：左边的 `?` 不能超出窗口，要根据位置自动判断向右或向左显示。

## 9. 内置模板行为

当前根目录有：

```text
templates/
  字节核心MAX.json
```

`App.vue` 使用类似下面的方式加载内置模板：

```ts
import.meta.glob("../templates/*.json", { eager: true, import: "default" })
```

含义：

- 根目录 `templates` 文件夹内所有 `.json` 模板会在构建时内置进应用。
- 软件内可以通过“内置模板”下拉选择这些模板。
- 用户也可以通过“保存模板”和“读取模板”导出 / 导入 JSON 文件。

注意：这些内置模板是构建时打包进去的，build 后再往文件夹加模板，不会自动出现在已构建的 exe 里，除非重新 build。

## 10. 当前保存方式

当前 Tauri + Vue 版本主要使用 WebView 本地存储：

```text
localStorage
```

当前 README 中提到的键：

```text
give-generator-pyside-autosave
give-generator-animation
```

含义：

- `give-generator-pyside-autosave`：自动保存表单草稿。
- `give-generator-animation`：界面动画开关。

注意：虽然键名里还有 `pyside`，但这是历史命名，不代表当前项目仍是 PySide6。

未来计划可以接入 Tauri 文件系统能力，把模板和配置迁移到应用数据目录。

## 11. 构建验证

近期修改后执行过：

```powershell
npm run build
```

构建通过。最近一次已知输出大致为：

```text
✓ 26 modules transformed.
dist/index.html
dist/assets/index-*.css
dist/assets/index-*.js
✓ built
```

文档类修改不需要重新 build。改 Vue/TS/CSS 后应运行 `npm run build` 验证。

## 12. 当前 Git 工作区状态提示

生成这份总结前，工作区有未提交改动，包括：

```text
src/App.vue
src/components/EffectEditor.vue
src/components/RichTextEditor.vue
src/style.css
src/components/CatalogCombo.vue
src/components/CustomSelect.vue
src/components/InfoTip.vue
src/components/NumberInput.vue
templates/
```

这些是前面 UI 和模板相关修改，不要随意回退。

## 13. 未来继续修改时的注意事项

- 不要恢复原生 `<select>`、`<datalist>`、`input[type=number]` 的默认控件样式。
- 新增菜单、弹窗、提示层时优先 Teleport 到 `body`，避免被 `overflow` 裁切。
- Tooltip 要根据 viewport 自动选择显示方向。
- 左侧面板、物品选择、补全菜单要保持可滚动，不能超出窗口。
- 视觉风格继续保持深蓝玻璃风，不要出现突兀的白色系统菜单。
- 不要把 PySide6 的实现细节写回 README，除非是在历史说明里。
- 修改生成逻辑时必须注意 Java / Bedrock 差异，特别是 `minecraft:` 前缀和组件格式。
- 修改模板加载时记住：`templates/*.json` 是构建时内置。

## 14. 后续开发计划

README 里的开发计划方向包括：

- 补全物品数据库。
- 补全方块数据库。
- 补全附魔中文数据。
- 补全属性中文数据。
- 增加更多 Minecraft Java 版本。
- 增加更多 Bedrock 兼容规则。
- 增加刷怪蛋数据支持。
- 优化模板系统。
- 接入 Tauri 文件系统能力，支持应用数据目录模板管理。
- 优化动画与交互细节。
- 增加更多命令组件。

旧总结中还提到：后续 4.0 可继续做 Minecraft 1.21+ 所有版本适配。

## 15. 给新对话的开场提示

如果要在新对话继续，可以直接说：

```text
继续 Give 指令生成器项目。项目已经从 PySide6 迁移到 Tauri v2 + Vue 3 + TypeScript。当前核心文件是 src/App.vue、src/style.css、src/logic/builder.ts、src/data/catalog.ts，组件包括 CustomSelect、CatalogCombo、InfoTip、NumberInput、RichTextEditor、EffectEditor。请基于根目录的 give_command_generator_updated_conversation_summary.md 继续，不要把项目当成 PySide6。
```

## 16. 本轮总结生成时间

```text
2026-06-04
```

