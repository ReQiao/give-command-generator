[33m0995d6b[m Revert "revert: 重置源码到 decb636，用于构建不依赖服务器的版本"
[33mc570127[m revert: 重置源码到 decb636，用于构建不依赖服务器的版本
[33mb26d4c3[m 把液态玻璃折射材质搬进真实界面
[33m6e67e91[m 物品选择弹窗改成从「选择…」按钮位置流出的液态形变动画
[33m6a0dbda[m fix: 基岩版改用基岩 ID 表，补齐 Java 两个诅咒附魔
[33mc625bdb[m fix(ui): 每次切进 AI 模式都重放点灯特效，不只是第一次
[33mef6f49b[m feat: 加入使用须知（EULA）弹窗，必须滑到底并同意才能进入软件
[33ma5902fa[m feat(deploy): 部署时识别存档实际版本，和当前选择不一致给出提示
[33m4e9ee27[m fix(ai): 拦截 AI 编造的物品/方块/实体/附魔/效果/属性 id，修复面板状态丢失
[33m747ba41[m revert: 恢复源码到最新版（撤销临时回退到 16f9244 的构建用状态）
[33md40b5a0[m[33m ([m[1;33mtag: [m[1;33mv4.1[m[33m, [m[1;31mgitee/main[m[33m)[m revert: 重置源码到 16f9244，用于构建该提交对应的稳定版安装包
[33m88f9a1f[m docs(ui): 记录"手动/AI 模式切换不能用 Transition"的实测结论
[33mf5bc3f9[m fix(ui): 去掉手动/AI 模式切换的淡入淡出过渡，改成原子切换
[33mca94ae5[m fix(ui): 彻底修复手动模式切到 AI 模式时界面瞬间下移又弹回
[33m56ac536[m feat(deploy): 存档手动选择 + 手动模式部署 + 循环效果自动挂 tick.json
[33mb975af0[m fix(ui): AI 模式去掉圆圈能量环特效，手动模式切回时加基础淡入动画
[33m8df528a[m feat(ui): AI 模式加入启动特效，「自己填」改名为「手动模式」
[33mba7fd72[m feat(ui): 顶部加「自己填 / AI 模式」切换，新增 AI 生成与一键部署面板
[33m2b718f2[m feat(catalog): 用官方数据源补全物品表，新增分类物品选择弹窗
[33m121524c[m feat(template): 保存模板时弹出原生"另存为"对话框，提示已保存路径
[33m2c9178e[m fix(ui): 恢复左上角渐变色 logo 容器，去掉里面的 emoji
[33me65aa33[m revert(ui): 窗口内部标题与 logo 改回原样
[33m6c7959a[m feat(rebrand): 应用改名为 Soul Lantern（灵魂灯笼），版本升至 4.1.0
[33m0a2fa9c[m feat(rich-text): 完整文本组件系统（字体/object/事件/高级类型）
[33mef97e01[m fix: 修正版本号和组件兼容性显示
[33m9d7ebbb[m feat: 适配组件时代全版本族（1.20.5~26.2+）
[33mdc6600f[m feat: 新增 Java 1.21.2 / 1.21.3 适配（mid 语法族）
[33mba23d41[m fix: 修复 Java 1.21/1.21.1 缺少 item_name 输出与 UI 隐藏问题
[33mabf01ac[m release v3.4
[33m5928066[m Initial Tauri Vue give command generator
