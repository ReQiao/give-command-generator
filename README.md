# Give指令生成器

一个面向 Minecraft 的 `/give` 指令生成器，当前支持 **Java 1.21.11+** 与 **基岩版** 两种模式。

***本版本对项目进行了大规模重构与功能扩展，已全面转为 **Tauri v2 + Vue 3 + TypeScript** 架构

这是一个基于 **Tauri v2 + Vue 3 + TypeScript** 的桌面应用，目标是解决 MC 新手以及指令熟手在编辑 `/give` 指令 JSON 和物品组件时步骤繁琐、容易写错的问题。界面为中文深蓝玻璃风 UI，支持富文本、渐变颜色、阴影颜色、模板导入导出、自动保存和多版本指令生成。

**注意：如果下载后被杀软拦截，请您放心同意运行，本程序没有任何病毒。**

## 功能特性

- 中文界面
- 深蓝玻璃风 UI
- Tauri 桌面窗口，不需要部署网站
- Java 1.21.11+ `/give` 组件语法
- 基岩版 `/give` 基础组件语法
- 版本选择：Java 1.21.11+ / 基岩版
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
- 点击“生成指令”后生成最终指令
- 一键复制指令
- 深蓝玻璃风弹窗
- 中文颜色选择器
- Toast、页面切换、按钮反馈、表格高亮等界面动画

## 当前支持版本

### Java 版

```text
Java 1.21.11+
```

当前主要支持新版物品组件格式，例如：

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
- 当前版本没有使用 `%APPDATA%\Give指令生成器\templates\` 作为模板目录

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

打包产物通常在：

```text
src-tauri/target/release/bundle/
```

Windows 安装包一般在：

```text
src-tauri/target/release/bundle/nsis/
```

直接运行的程序本体一般在：

```text
src-tauri/target/release/give-command-generator.exe
```

## 注意事项

Minecraft 指令组件语法会随版本变化。

本项目当前以实测语法为准，不保证所有 Minecraft 版本都兼容。  
如果某个组件在游戏中报错，请以游戏实际提示为准，并提交可复现的正确指令和错误指令。

## 开发计划

- 补全物品数据库
- 补全方块数据库
- 补全附魔中文数据
- 补全属性中文数据
- 增加更多 Minecraft Java 版本
- 增加更多基岩版兼容规则
- 增加刷怪蛋数据支持
- 优化模板系统
- 接入 Tauri 文件系统能力，支持应用数据目录模板管理
- 优化动画与交互细节
- 增加更多命令组件
