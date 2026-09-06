[33m6a3c6ae[m[33m ([m[1;36mHEAD[m[33m -> [m[1;32mmain[m[33m, [m[1;31morigin/main[m[33m, [m[1;31morigin/HEAD[m[33m)[m feat: 恢复客户端可选模型
[33ma87f559[m feat(client): 接入真实生产证书，替换占位符
[33m96b28af[m docs(server): 部署清单改成 Windows 本机适配版
[33m2e1fc3e[m feat(client): 接入远程账本 + AI 代理，彻底去掉本地/BYOK 大模型 key
[33mdecb636[m fix(server): 修复证书锁定的 CaUsedAsEndEntity 坑，补真实 TLS 集成测试
[33m8b984f5[m docs(server): 加 systemd 配置 + 部署清单
[33m2337bff[m feat(server): 新增账本 + AI 调用代理服务端
[33mf017a64[m feat(billing): 给本地计费状态加 HMAC 签名，防手改 JSON 余额
[33m056d379[m feat(billing): 余额/激活状态落盘 + 激活码兑换 UI
[33m9f247c8[m fix(ai): 补齐 6 处漏掉的目录校验
[33mf4b872d[m fix: 基岩版 AI 模式禁用 + 目录校验版本感知 + 粒子目录校验
[33m6a0dbda[m fix: 基岩版改用基岩 ID 表，补齐 Java 两个诅咒附魔
[33mc625bdb[m fix(ui): 每次切进 AI 模式都重放点灯特效，不只是第一次
[33mbde358f[m chore: 修复 nanoid 高危漏洞（npm audit fix）
[33mef6f49b[m feat: 加入使用须知（EULA）弹窗，必须滑到底并同意才能进入软件
[33m8e5e570[m build: 生产构建加 JS 混淆，提高逆向门槛
[33m8468638[m feat(give): 支持 custom_data 组件，修复爆炸箭误伤所有箭的问题
[33m616cb70[m test(mc-verifier): 新增探针，验证发射的箭是否继承物品 custom_data 组件
[33m855f19b[m fix(ai): 更新 DeepSeek V4 Pro 真实价格（截图确认）
[33maa2a3d3[m feat(billing): AI 面板加充值入口，按真实 token 用量折算灵魂币扣费
[33ma5902fa[m feat(deploy): 部署时识别存档实际版本，和当前选择不一致给出提示
[33m8af2c29[m feat(ai): 支持多轮上下文续聊，封顶3轮自动开始新对话
[33m4e9ee27[m fix(ai): 拦截 AI 编造的物品/方块/实体/附魔/效果/属性 id，修复面板状态丢失
[33m1533416[m fix(ai): 关闭 qwen3.7 混合推理模型的默认思考模式
[33m1592499[m feat(ai): 内置临时测试用 key + 接入自定义百炼工作空间端点，AI面板加模型下拉
[33m747ba41[m revert: 恢复源码到最新版（撤销临时回退到 16f9244 的构建用状态）
[33md40b5a0[m[33m ([m[1;33mtag: [m[1;33mv4.1[m[33m, [m[1;31mgitee/main[m[33m)[m revert: 重置源码到 16f9244，用于构建该提交对应的稳定版安装包
[33m88f9a1f[m docs(ui): 记录"手动/AI 模式切换不能用 Transition"的实测结论
[33mf5bc3f9[m fix(ui): 去掉手动/AI 模式切换的淡入淡出过渡，改成原子切换
[33mca94ae5[m fix(ui): 彻底修复手动模式切到 AI 模式时界面瞬间下移又弹回
[33m56ac536[m feat(deploy): 存档手动选择 + 手动模式部署 + 循环效果自动挂 tick.json
[33m6dbb1ec[m fix(ai): 修复多项 AI 模式可靠性问题（血量/时长/朝向/粒子/判定实体/计分板）
[33m7e06525[m feat(ai): AI 模式支持任意 OpenAI 兼容服务商，不再绑死 DashScope
[33m092692f[m fix(ui): 关闭动画时手动模式切到 AI 模式仍会出现整页跳动
[33mb975af0[m fix(ui): AI 模式去掉圆圈能量环特效，手动模式切回时加基础淡入动画
[33m8df528a[m feat(ui): AI 模式加入启动特效，「自己填」改名为「手动模式」
[33mba7fd72[m feat(ui): 顶部加「自己填 / AI 模式」切换，新增 AI 生成与一键部署面板
[33mc5566c2[m feat(tauri): 新增 AI 调用、计费骨架与 datapack 一键部署三个后端模块
[33m6790181[m feat(ai): 新增系统提示词构建与响应解析，把实测机制真值写进 AI 指南
[33m6d01f55[m feat(mc-verifier): 实证「落地检测」组合技的 NBT 真值，并修复探针工具的三处坑
[33m0ea9ca2[m feat(commands): 新增多指令构建器与 AI 意图分派器
[33m16f9244[m fix(style): 弹窗尺寸改用比例 clamp，按用途区分大小，手机端贴边
[33m2b718f2[m feat(catalog): 用官方数据源补全物品表，新增分类物品选择弹窗
[33md993b2f[m feat(style): 增加手机宽度断点，修复窄屏下的横向溢出
[33m121524c[m feat(template): 保存模板时弹出原生"另存为"对话框，提示已保存路径
[33m2c9178e[m fix(ui): 恢复左上角渐变色 logo 容器，去掉里面的 emoji
[33mf9b4138[m fix(config): 窗口标题改为纯英文 Soul Lantern
[33me65aa33[m revert(ui): 窗口内部标题与 logo 改回原样
[33m50845cc[m fix(deps): 升级 vue-tsc 到 3.3.8 修复 brace-expansion 高危漏洞
[33m6c7959a[m feat(rebrand): 应用改名为 Soul Lantern（灵魂灯笼），版本升至 4.1.0
[33ma1709a3[m feat(icons): 全平台图标替换为灵魂灯笼贴图
[33m0ccc8ac[m fix(style): 拆分 .card/.modal-card 共享规则，避免弹窗透明度改动波及主界面
[33m9d5c01c[m +图
[33mfa02c9a[m chore: npm 缓存清理后重装依赖
[33mff3bbab[m fix(rich-text): 标签改为纯中文并补充 ? 说明，修正提示气泡箭头未对准
[33m0a2fa9c[m feat(rich-text): 完整文本组件系统（字体/object/事件/高级类型）
[33m2f09bd2[m fix(ci): linux 打包 AppImage 缺少 xdg-utils 导致 xdg-open 报错
[33m2243790[m ci: 新增 give-buildall-other workflow（对齐 build_release 脚本约定）
[33m8e0b38e[m feat(give-builder): 新增一键编译发布 tkinter 脚本
[33m5c247ab[m Update Java version in README.md
[33m8393d2c[m Delete .github/workflows directory
[33maeaebd5[m[33m ([m[1;33mtag: [m[1;33mv4.0[m[33m)[m Update Java version in README.md
[33m6d3b0c9[m Update README to clarify project focus
[33m823ec16[m 重构 README.md 以反映新架构和功能
[33me6b411a[m Refactor README to remove duplicate antivirus warning
[33m19c69f4[m Merge branch 'main' of https://github.com/ReQiao/give-command-generator hi
[33mef97e01[m fix: 修正版本号和组件兼容性显示
[33m7b006c5[m Resolve merge conflict in README.md
[33m043d7aa[m feat: 服务器实证确认 Java 26.1 / 26.2+ 为 modern 族
[33m9d7ebbb[m feat: 适配组件时代全版本族（1.20.5~26.2+）
[33mdc6600f[m feat: 新增 Java 1.21.2 / 1.21.3 适配（mid 语法族）
[33m969fc5e[m fix: 修正 can_place_on/can_break/属性id/tooltip 的版本相关语法
[33mfdf7c7d[m feat: 新增 MC give 语法自动验证器（真实服务器 + RCON）
[33mba23d41[m fix: 修复 Java 1.21/1.21.1 缺少 item_name 输出与 UI 隐藏问题
[33m44eafca[m[33m ([m[1;33mtag: [m[1;33mv3.4[m[33m)[m Add GitHub Actions workflow for Linux ARM64 build
[33m508120b[m Add GitHub Actions workflow for Linux x64 build
[33m8269289[m Update GitHub Actions to support macOS 15 and ARM64
[33m21407ba[m Rename workflow from build-mac-x64 to build-mac-silicon
[33m1cf5123[m Update build-mac-silicon.yml
[33m8bbc29a[m Create build-mac-silicon.yml
[33m26a9297[m release v3.4
[33m2ad1df2[m 删除文件 .vscode
[33m7a4f90a[m 更新README以适配最新版
[33mabf01ac[m release v3.4
[33mbb5b4bc[m add tauri icon files
[33m6bfbc41[m update github actions node24 versions
[33m29dd100[m add mac x64 build workflow
[33me040bce[m Merge remote-tracking branch 'origin/main'
[33ma6fbe62[m configure github remote and git attributes
[33maf48ea3[m[33m ([m[1;33mtag: [m[1;33mv3.3[m[33m, [m[1;33mtag: [m[1;33mv3.2[m[33m)[m Update README.md
[33m67b3284[m Update README.md
[33mb022aed[m Update README.md
[33mac4d97d[m Fix gitignore conflict markers
[33ma898072[m Update README.md
[33m1b21881[m Delete .vscode directory
[33me0593d6[m LOL
[33mab9574a[m LOL
[33m5928066[m Initial Tauri Vue give command generator
[33m089e958[m Initial commit
