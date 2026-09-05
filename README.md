# Soul Lantern（灵魂灯笼）

> ## ⚠️ 这是 `demo/local-ai` 分支，不要合进 main
>
> 这个分支为**录制演示视频**而存在：AI 模式不连任何服务器，指令构建器搬回了
> 前端（`src/logic/dispatch.ts` + `src/logic/commands/`），API key 由用户自己
> 在界面上填。不需要服务端二进制、不需要环境变量、断网可用。
>
> 主线把构建器放在服务器、把 key 收到服务器，是拿事故换来的结论
> （见 `src-tauri/src/ai.rs` 顶部）。这个分支只是把界面完整地拍下来的手段，
> 不是产品方向。账号、灵魂币、充值、兑换码在这里整块拿掉了。
>
> 界面本身和主线完全一致——这正是建这个分支而不是回退旧 commit 的原因。


一个面向 Minecraft 的 `/give` 指令生成器，当前支持 **Java 1.21.11+** 与 **基岩版** 两种模式。

**注意：如果下载后被杀软拦截，请您放心同意运行，本程序没有任何病毒。**

**本版本对项目进行了大规模重构与功能扩展，已全面转为 `Tauri v2 + Vue 3 + TypeScript` 架构**

这是一个基于 **Tauri v2 + Vue 3 + TypeScript** 的桌面应用，目标是解决 MC 新手以及指令熟手在编辑 `/give` 指令 JSON 和物品组件时步骤繁琐、容易写错的问题。界面为中文深蓝玻璃风 UI，支持富文本、渐变颜色、阴影颜色、模板导入导出、自动保存和多版本指令生成。

## 功能特性

- 几乎完整的物品，附魔，属性数据库
- 中文界面
- 深蓝玻璃风 UI
- Java 1.20.5+ `/give` 组件语法
- 基岩版 `/give` 基础组件语法
- 版本选择：Java 1.20.5 ~ 26.2 / 基岩版
- 显示名称、物品名称、物品描述富文本编辑
- 加粗、斜体、下划线、删除线
- 文字颜色与渐变颜色
- 阴影颜色与透明度
- 附魔生成
- 属性修饰符生成
- 可放置 / 可破坏方块限制
- 基础组件生成
- 食物组件生成
- 消耗组件与食用效果生成
- 死亡保护与死亡效果生成
- 工具规则生成
- 基岩版数据值、物品锁、死亡保留
- 自动保存草稿
- JSON 模板导出与导入
- 内置 JSON 模板
- 点击“生成指令”后生成最终指令
- 一键复制指令
- 深蓝玻璃风弹窗
- Toast、页面切换、按钮反馈、表格高亮等界面动画
- "?"悬浮解释选项

## 当前支持版本

### Java 版

```text
Java 1.20.5+
```

当前支持新版物品组件格式，例如：

```mcfunction
give @a minecraft:stone[custom_name=[{"text":"石头","color":"#7aa2ff"}],unbreakable={}] 1
```

### 基岩版

```text
Bedrock
```

当前基岩版主要支持基础 `/give` 格式：

```mcfunction
/give @a cobblestone 1 0 {"minecraft:can_place_on":{"blocks":["stone"]}}
```

基岩版暂不直接支持 Java 版的富文本、属性、食物效果等组件语法。

## 保存方式

当前 Tauri + Vue 版本使用 WebView 本地存储保存草稿和界面设置：

```text
localStorage
```

当前使用的本地存储键：

```text
give-generator-pyside-autosave
give-generator-animation
```

说明：

- `give-generator-pyside-autosave`：自动保存的表单草稿
- `give-generator-animation`：界面动画开关
- 模板通过软件内的“保存模板”和“读取模板”导出 / 导入 JSON 文件
- 当前版本已停止使用 `%APPDATA%\Give指令生成器\templates\` 作为模板目录

## Java 版已支持组件

当前 Java 1.21.11+ 模式支持：

```text
custom_name
item_name
lore
rarity
enchantment_glint_override
enchantments
attribute_modifiers
can_place_on
can_break
unbreakable
glider
death_protection
damage
max_damage
max_stack_size
repair_cost
tooltip_display
food
consumable
on_consume_effects
tool
tool.rules
```

## 基岩版已支持内容

当前基岩版模式支持：

```text
物品 ID
数量
数据值
can_place_on
can_destroy
item_lock
keep_on_death
```

## 模板文件

模板使用 JSON 文件保存，可以通过软件内的“保存模板”和“读取模板”导入导出。

示例用途：

- 保存常用 OP 物品
- 保存测试指令
- 保存不同版本配置
- 分享给其他用户

## 开发运行

```powershell
npm install
npm run tauri dev
```

## 打包 Windows 安装包

```powershell
npm run tauri build
```

打包产物在：

```text
src-tauri/target/release/bundle/
```

Windows 安装包在：

```text
src-tauri/target/release/bundle/nsis/
```

直接运行的程序本体在：

```text
src-tauri/target/release/soul-lantern.exe
```

## 注意事项

Minecraft 指令组件语法会随版本变化。

本项目当前以实测语法为准，不保证所有 Minecraft 版本都兼容。  
如果某个组件在游戏中报错，请以游戏实际提示为准，并提交可复现的正确指令和错误指令。

## 界面出问题时的应急开关

界面的液态玻璃（面板/弹窗边缘那圈折射）用的是 `backdrop-filter` 引用 SVG 滤镜，
只有 Chromium 内核支持。macOS 版用的是系统的 WKWebView，会自动退回纯模糊，
观感上差别很小。

万一某台机器上玻璃显示异常（显卡驱动、某个 WebView2 版本），不用等新版本，
在开发者工具的控制台里敲一行然后刷新即可强制退回纯模糊：

```js
localStorage.setItem('soul-lantern-glass', 'off')   // 强制纯模糊
localStorage.setItem('soul-lantern-glass', 'on')    // 强制开折射
localStorage.removeItem('soul-lantern-glass')       // 恢复自动判断
```

这个开关同时也是**验证降级效果的唯一手段**：设成 `off` 看到的界面，
和 macOS 用户看到的完全一样，不需要真有一台 Mac。

另有一个测试开关，用来在浏览器/开发环境里走通"未登录点 AI 模式"那条分支
（正常情况下它需要同时满足"跑在桌面端"和"服务端要求登录"，开发时两条都不成立）：

```js
localStorage.setItem('soul-lantern-gate', 'on')     // 强制显示登录门禁
localStorage.removeItem('soul-lantern-gate')        // 恢复正常
```

它只会**多加**一道门禁，不会绕过任何鉴权。

## 开发计划

- 增加更多 Minecraft Java 版本
- 增加更多基岩版兼容规则
- 增加刷怪蛋数据支持
- 优化模板系统
- 优化动画与交互细节
- 增加更多命令组件
