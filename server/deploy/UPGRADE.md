# 升级与故障处理

`RUNBOOK.md` 讲的是**从零全新安装**。这份讲的是**服务已经在跑了，怎么换一版
新的二进制**，以及出事之后怎么救。

---

## ⚠️ 先读这三条

**1. 从这一版（schema v2）开始，回滚二进制必须连账本一起回滚。**

老版本的服务端只认识 `accounts` 一张表，serde 默认忽略不认识的字段——意思是
老二进制**读得进**新账本，然后把 `users` / `sessions` / `pending` 三张表
静默丢掉，下一次写操作（任何人查一次余额、生成一次指令）就把整张用户表
永久抹掉。它不会报错，也不会拒绝启动。

所以回滚的正确姿势是**四步一体**，顺序不能变：

```bash
systemctl stop soul-lantern
cp /opt/soul-lantern/soul-lantern-server.old /opt/soul-lantern/soul-lantern-server
cp /opt/soul-lantern/backups/ledger-boot-s1-<对应时间戳>.json /opt/soul-lantern/ledger.json
chown soul-lantern:soul-lantern /opt/soul-lantern/ledger.json
systemctl start soul-lantern
```

备份文件名里的 `s1`/`s2` 就是账本格式版本，回滚时按这个挑，别只看时间戳。

**2. 手动改 `ledger.json` 之前必须先停服务。**

内存里那份是权威，磁盘只是它的投影。服务跑着的时候你 `nano` 改完保存，
三十秒后随便谁来一次请求，内存就把你改的内容整个覆盖回去了——而且毫无提示。

```bash
systemctl stop soul-lantern
nano /opt/soul-lantern/ledger.json
chown soul-lantern:soul-lantern /opt/soul-lantern/ledger.json
systemctl start soul-lantern
```

**3. `backups/` 里是全量手机号、密码哈希、会话摘要。**

敏感等级等同 `.env`。目录已经建成 0700 了，但你 `scp` 拉到 Windows 桌面上之后
就没有这层保护了——别顺手丢进网盘、别发给任何人（包括发给我）。

---

## 在哪编译

服务器上没有 Rust 工具链，二进制是在别的机器上交叉编译好再传上去的。
产物是**静态链接的 musl 二进制**，不依赖服务器上任何系统库。

```bash
# 一次性准备
rustup target add x86_64-unknown-linux-musl
apt-get install -y musl-tools        # 提供 musl-gcc

# 每次编译
cd server
CC_x86_64_unknown_linux_musl=musl-gcc \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
cargo build --release --target x86_64-unknown-linux-musl

# 产物
ls -lh target/x86_64-unknown-linux-musl/release/soul-lantern-server
file   target/x86_64-unknown-linux-musl/release/soul-lantern-server   # 应该写着 statically linked
```

编完顺手确认一下 C 依赖没有偷偷混进来：

```bash
cargo tree | grep -i aws-lc     # 应该什么都不输出
```

（`axum-server` 的 `tls-rustls` feature 会拖进 `aws-lc-sys`，那是一坨需要
C 编译器和 cmake 的东西，和"纯 Rust 好交叉编译"的选型冲突。`Cargo.toml` 里
已经改用 `tls-rustls-no-provider` 了，这条命令是防止以后有人改回去。）

---

## 升级流程（固定四步，别跳步）

关键在于**先自检、后覆盖**。`--check` 会读全部环境变量、解析账本、验证证书
可读、检查目录可写，全部通过才 exit 0，而且**不绑定端口**——所以老服务还
跑着的时候跑它是安全的。

```powershell
# 第 1 步：传上去，先不覆盖，用 .new 后缀
scp .\soul-lantern-server root@120.26.175.121:/opt/soul-lantern/soul-lantern-server.new
```

```bash
# 第 2 步：用运行服务的那个账号自检（用 root 跑会看不出权限问题）
chmod +x /opt/soul-lantern/soul-lantern-server.new
sudo -u soul-lantern /opt/soul-lantern/soul-lantern-server.new --check
```

输出长这样，任何一行 `[FAIL]` 都**不要往下走**：

```
== 灵魂灯笼服务自检 ==
  [OK]   AI_ENDPOINT：https://...
  [OK]   AUTH_PEPPER：已设置（64 字符）
  [OK]   SMS_KIND：aliyun
  [FAIL] SMS_SIGN_NAME：未设置（SMS_KIND=aliyun 时必填）
  ...
自检未通过——**不要**覆盖现有二进制，先把上面 [FAIL] 的项补齐。
```

```bash
# 第 3 步：留一份旧的（回滚要用），再覆盖
cp /opt/soul-lantern/soul-lantern-server /opt/soul-lantern/soul-lantern-server.old
mv /opt/soul-lantern/soul-lantern-server.new /opt/soul-lantern/soul-lantern-server
chown soul-lantern:soul-lantern /opt/soul-lantern/soul-lantern-server

# 第 4 步：重启并盯一眼日志
systemctl restart soul-lantern
journalctl -u soul-lantern -f
```

看到「灵魂灯笼服务监听 0.0.0.0:8443」就是起来了，`Ctrl+C` 退出日志查看。
服务每次启动都会自动往 `backups/` 写一份升级前的账本快照，不用手动备份。

---

## 首次升级到 schema v2 要补的环境变量

这一版加了账号体系，`.env` 需要新增几项。**`AUTH_PEPPER` 不填服务起不来**
（这是故意的——它是账号体系的根密钥，用一个空值跑起来比起不来危险得多）。

```bash
nano /opt/soul-lantern/.env
```

照 `.env.example` 补上：`AUTH_PEPPER`、`SMS_KIND`、`SMS_ACCESS_KEY_ID`、
`SMS_ACCESS_KEY_SECRET`、`SMS_SIGN_NAME`、`RUST_LOG`。
建议顺手把 `ADMIN_TOKEN` 也设上，排障时很有用。

生成随机密钥：

```bash
openssl rand -base64 48    # AUTH_PEPPER
openssl rand -base64 32    # ADMIN_TOKEN
```

---

## 发激活码

激活码的校验位是用 `AUTH_PEPPER` 算的 HMAC，所以**必须在配好 .env 的这台
服务器上生成**，别的地方生成的码服务器不认。

```bash
cd /opt/soul-lantern
set -a; . ./.env; set +a          # 把 .env 读进环境变量
./soul-lantern-server --gen-license 20 > /root/codes.txt
```

出来的是 `SOUL-XXXX-YYYY-ZZZZ` 这种形态，一行一个。字符集去掉了 I/O/0/1
（手抄最容易看错的四个）。每个码全局只能兑换一次，兑过之后任何账号再兑都会被拒。

---

## 故障手册

### 服务起不来，日志里全是 HALT

账本文件出问题了，服务**故意**停在这里不上线。

这个设计是刻意的：如果损坏时就当空账本启动，所有人的余额会静默清零，而且
日志里什么都看不出来。宁可服务一直是 down 的让你立刻发现。

```bash
cat /opt/soul-lantern/HALT       # 里面直接写了处置步骤，照着敲
```

大致是：先 `cp` 一份坏档留证 → 从 `backups/` 挑一份最近的 → 覆盖回
`ledger.json` → `chown` → **删掉 HALT 文件** → restart。
不删 HALT 的话服务每次启动都会立刻退出，这也是故意的。

### 用户说收不到验证码

按这个顺序查，从最省事的开始：

```bash
# 1. 先确认是不是全局配额被刷满了（被刷时日志里有 error! 级别的整行提示）
curl -H "Authorization: Bearer <ADMIN_TOKEN>" \
     --cacert server.crt "https://120.26.175.121:8443/v1/admin/stats"
# 看 smsSentToday 和 smsDailyCap

# 2. 直接发一条测试短信给自己，这个接口会把阿里云的原始报错原样透出来
curl -H "Authorization: Bearer <ADMIN_TOKEN>" \
     --cacert server.crt "https://120.26.175.121:8443/v1/admin/sms-test?to=你的手机号"

# 3. 查这个用户在不在
curl -H "Authorization: Bearer <ADMIN_TOKEN>" \
     --cacert server.crt "https://120.26.175.121:8443/v1/admin/lookup?q=13800138000"
```

> **为什么 admin 只读接口都是 GET + query 而不是 POST + JSON**：
> Windows PowerShell 5.1 把参数传给原生 exe 时会剥掉内层双引号，
> `curl.exe -d '{"to":"x"}'` 实际发出去的是 `{to:x}`，服务器返回 400——
> 而你需要用这些接口的时刻，恰恰是"用户收不到验证码、我要赶紧定位"的
> 高压时刻，那时候排查一个和"代码写错了"无法区分的错误最要命。
>
> 唯一改状态的 `/v1/admin/user` 保留 POST，用它的时候**不要写内联 JSON**，
> 用记事本存一个 `body.json` 然后 `curl.exe -d "@body.json"`。

**救火通道**：如果短信一时半会修不好（欠费、key 失效、阿里云那边故障），
先让注册流程能走：

```bash
nano /opt/soul-lantern/.env      # SMS_KIND 改成 log
systemctl restart soul-lantern
journalctl -u soul-lantern -f    # 用户注册后，验证码会打在这里
```

让用户把手机号报给你，你从日志里捞码告诉他。修好之后记得改回 `aliyun`。

再退一步，如果连注册都不想让人卡着，把 `AUTH_REQUIRED=false` 一设，
客户端就不挡登录门禁了（服务端仍然会 401 挡住真实调用，只是界面不再是
一堵墙）。

### 充值/扣费"成功"了但余额没变

多半是账本写不进去。最常见的原因是从备份恢复之后忘了 `chown`——文件属主
变成 root，而服务以 `soul-lantern` 跑，能读不能写。

新版本已经在启动时就会自检目录可写并直接 HALT，所以这个问题现在应该在启动
阶段就暴露出来了。如果是运行中变成不可写（比如磁盘满），看这里：

```bash
curl -H "Authorization: Bearer <ADMIN_TOKEN>" \
     --cacert server.crt "https://120.26.175.121:8443/v1/admin/stats"
# lastPersistOk 是 false 就说明有写失败发生过
df -h /opt                       # 磁盘满没
ls -l /opt/soul-lantern/         # 属主对不对
```

### 日志里什么都看不到

`RUST_LOG` 没设。这一版给 tracing 开了 `env-filter`，而它在 `RUST_LOG`
未设置时默认**只放行 ERROR**——启动日志、审计日志、`SMS_KIND=log` 的验证码
会一起被静音。`.env` 里加上 `RUST_LOG=info` 再 restart。

另外确认一下 journald 是不是 volatile 模式（不少阿里云 Ubuntu 镜像没有
`/var/log/journal` 目录，日志只在内存里、重启即失）：

```bash
journalctl --disk-usage
# 如果显示的容量很小或者报错，建一个持久化目录：
mkdir -p /var/log/journal && systemctl restart systemd-journald
```

---

## 备份

服务会自己写，不用你记得：

- `backups/ledger-boot-s2-<时间戳>.json` —— 每次启动写一份（也就是每次升级
  前一刻的原样），保留最近 5 份
- `backups/ledger-auto-s2-<时间戳>.json` —— 运行中每 6 小时一份，保留 28 份
  （约 7 天）

但这些都在同一台服务器上，机器整个挂了就一起没了。**定期往别处拉一份**：

```powershell
scp -r root@120.26.175.121:/opt/soul-lantern/backups .\soul-lantern-backups
```

拉下来的文件含手机号和密码哈希，注意保管（见开头第 3 条）。
