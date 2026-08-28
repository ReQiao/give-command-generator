# 部署清单：一步步在你的阿里云 ECS 上跑起来（Windows 本机版）

全程你自己在自己的终端里操作（SSH 连你的服务器），密码/私钥全程不经过我。
我会把编译好的二进制文件发给你，其余步骤都是复制粘贴命令。

服务器本身是 Ubuntu，只要 SSH 连上去了、后续在服务器里敲的命令（第 1～5 步）
全都是远程 Linux 环境里的 bash，跟你自己电脑是 Windows 没有任何关系，照抄
不用改。真正跟"你是 Windows"有关系的只有第 0 步（从本机传文件）和第 6 步
（从本机验证），下面这两步给了 Windows 专用的写法。

**终端用什么**：推荐用 **Windows Terminal**（Win11 自带，Win10 可以在
微软商店免费装）或者直接用系统自带的 **PowerShell**，都能用。不推荐老式的
`cmd.exe`（部分命令语法不兼容）。

**先确认自带了 SSH 客户端**：Windows 10（1809 之后）和 Windows 11 都默认
自带了 OpenSSH 客户端，`ssh`/`scp` 直接能用，一般不用额外装任何东西。打开
PowerShell 敲一下确认：

```powershell
ssh -V
```

能看到类似 `OpenSSH_for_Windows_...` 这样的版本号就说明有；如果提示"不是
内部或外部命令"，去「设置 → 应用 → 可选功能 → 添加功能」搜索
"OpenSSH 客户端" 装上（不用重启），或者用下面提到的 WinSCP 图形界面工具
代替命令行操作。

**你是用密码登录服务器**（不是密钥），下面每一条 `ssh`/`scp` 命令敲回车后
都会停下来问你密码——这是正常的，不是卡住了：
- 输入密码的时候屏幕上**不会显示任何字符**（连星号 `*` 都没有），这是
  Linux/OpenSSH 一贯的行为，正常打字然后回车就行，不是你的电脑坏了。
- 第一次连这台服务器，可能会先弹出一段类似
  "The authenticity of host ... can't be established... Are you sure you
  want to continue connecting (yes/no)?" 的英文提示，这也是正常的（只在
  第一次连接时出现），直接打 `yes` 回车，再输密码。

## 第 0 步：把二进制传到服务器

我会把 `soul-lantern-server`（已经编译好、静态链接，不需要服务器上装 Rust）
发给你，下载到你电脑上随便一个位置，比如 `C:\Users\你的用户名\Downloads\`。

打开 PowerShell，`cd` 到这个文件所在的目录，然后：

```powershell
scp .\soul-lantern-server root@120.26.175.121:/tmp/
```

（`scp` 命令本身在 PowerShell 里跟在 Linux/Mac 终端里写法完全一样，只是
路径前面 Windows 习惯加个 `.\`，不加也一样能跑。）

**如果不想用命令行**：装一个 [WinSCP](https://winscp.net/)（免费、图形界面），
连接方式选 SFTP，主机名填 `120.26.175.121`，用户名 `root`，输入密码连接后，
直接把 `soul-lantern-server` 这个文件拖到服务器的 `/tmp/` 目录就行，效果
一样，纯拖拽不用记命令。

## 第 1 步：SSH 上服务器，创建运行账号和目录

接下来的命令都是在**服务器上**执行（先 `ssh root@120.26.175.121` 连上去）。

```bash
# 建一个专门跑这个服务的低权限账号，不用 root 直接跑
useradd -r -s /usr/sbin/nologin soul-lantern

mkdir -p /opt/soul-lantern
mv /tmp/soul-lantern-server /opt/soul-lantern/
chmod +x /opt/soul-lantern/soul-lantern-server
```

## 第 2 步：在服务器上生成证书（私钥从此不离开这台机器）

最简单的办法：把仓库里 `server/certs/generate.sh` 的内容复制出来，在服务器上
粘贴保存成同名文件（不依赖仓库是公开还是私有，也不用在服务器上配 git 凭据）。
如果你更习惯直接在服务器上拉仓库，也可以 `git clone` 整个项目下来，用的就是
仓库里那一份，效果一样。

具体粘贴方法（在已经 SSH 连上服务器的窗口里）：

```bash
cd /opt/soul-lantern
nano generate.sh
```

`nano` 是个简单的文本编辑器，打开后是一片空白，直接在你自己电脑先复制好
`generate.sh` 的内容，然后**在这个终端窗口里点右键**（Windows Terminal 和
经典 PowerShell 窗口默认右键就是粘贴，`Ctrl+V` 在有些终端里不生效，右键
最保险），粘贴完之后按 `Ctrl+X` 退出，它会问要不要保存，按 `Y`，再回车确认
文件名，就存好了。

```bash
chmod +x generate.sh
./generate.sh 120.26.175.121
```

跑完这一步，`/opt/soul-lantern/` 下会多出 `server.crt`（公钥，等下要用）和
`server.key`（私钥，往后就一直留在这台服务器上，不要复制到别处）。

> 用的是仓库里当前这版 `generate.sh`（已经踩过一次坑修好了）：早期版本生成
> 的证书会被 openssl 默认标成 `CA:TRUE`，`curl --cacert` 校验不介意这个，
> 但客户端实际用的 rustls 校验器会直接拒绝、报 `CaUsedAsEndEntity`。
> 如果你手上的 `generate.sh` 比这个更早，务必重新拉一份最新的再生成。

**执行完之后，把 `server.crt` 的内容显示出来发给我**（这个文件不是秘密，
可以放心贴出来）：

```bash
cat server.crt
```

我拿到这段内容后会把它编进客户端代码里做证书锁定。

## 第 3 步：填真实配置

```bash
cat > /opt/soul-lantern/.env << 'EOF'
AI_API_KEY=把你的真实key填在这里
AI_ENDPOINT=https://ws-b2ui8x9tozwc8cq1.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/chat/completions
AI_MODEL=qwen-plus
TLS_CERT=/opt/soul-lantern/server.crt
TLS_KEY=/opt/soul-lantern/server.key
LEDGER_PATH=/opt/soul-lantern/ledger.json
BIND_ADDR=0.0.0.0:8443
EOF

# .env 里有真实 key，权限收紧成只有 root 能读
chmod 600 /opt/soul-lantern/.env

chown -R soul-lantern:soul-lantern /opt/soul-lantern
```

## 第 4 步：装成系统服务，开机自启、崩了自动重启

同样用 `nano` 打开一个新文件粘贴保存（粘贴方法同第 2 步：终端窗口里点右键，
`Ctrl+X` → `Y` → 回车保存）：

```bash
nano /etc/systemd/system/soul-lantern.service
```

粘贴仓库里 `server/deploy/soul-lantern.service` 的内容，保存退出后：

```bash
systemctl daemon-reload
systemctl enable --now soul-lantern

# 看一下起来没有
systemctl status soul-lantern
journalctl -u soul-lantern -f   # 看实时日志，Ctrl+C 退出
```

看到日志里出现「灵魂灯笼服务监听 0.0.0.0:8443」就是起来了。

## 第 5 步：开放端口

两个地方都要开，少一个都连不通：

1. **服务器自己的防火墙**（Ubuntu 默认可能没启用 ufw，先确认）：
   ```bash
   ufw status
   # 如果显示 active，要放行这个端口：
   ufw allow 8443/tcp
   ```
2. **阿里云控制台的安全组**——这个更容易漏掉：登录 ECS 控制台 → 找到这台实例 →
   「网络与安全组」→ 安全组规则 → 添加入方向规则，放行 TCP 8443，来源
   `0.0.0.0/0`（或者你想收紧成只允许特定来源也行）。

## 第 6 步：从你自己电脑验证

回到**你自己的 Windows 电脑**，把第 2 步里 `cat server.crt` 显示出来的内容
复制一份，在本机随便一个目录（比如刚才第 0 步用的那个文件夹）用记事本新建
一个文件，粘贴保存成 `server.crt`（内容不是秘密，之前已经贴给我了，你自己
留一份也行，方便以后随时验证服务器还活着）。

打开 PowerShell，`cd` 到这个文件所在目录，然后：

```powershell
curl.exe --cacert server.crt https://120.26.175.121:8443/v1/health
```

**注意这里必须写 `curl.exe`，不能只写 `curl`**——PowerShell 里 `curl` 这个
名字默认是 `Invoke-WebRequest` 的别名，跟真正的 curl 命令参数写法不一样，
`--cacert` 这个参数它不认，会报错。加上 `.exe` 后缀就是明确调用 Windows
自带的那个真正的 curl 程序，不会被别名接管。

看到返回 `ok` 就说明整条链路（证书、防火墙、安全组、服务本身）全部打通了。

如果提示"找不到 curl.exe"：说明你这台电脑的 Windows 版本比较老，自带的
curl 还没更新（2018 年之后的 Windows 10/11 都有）。这种情况可以直接用
WinSCP 连上去看看服务是不是真的在跑（`systemctl status soul-lantern`），
或者去 [curl 官网](https://curl.se/windows/) 下载一份单独装上。

## 以后要注意的事

- **`ledger.json` 是唯一的余额记录**，建议定期 `scp` 备份一份到别的地方
  （哪怕就是隔三差五手动备份一次），服务器万一出问题这是唯一救得回来的东西。
- **`AI_API_KEY` 只存在于服务器的 `.env` 文件里**，任何时候都不要把这个文件
  内容发出去（包括发给我）。真出问题（怀疑泄露/被盗刷）第一反应是去控制台
  撤销/换新 key，而不是先排查怎么泄露的。
- 这张自签名证书有效期给了 20 年，正常情况下不用管；如果哪天真要换（比如
  私钥不小心泄露了），流程是：服务器上重新跑一次 `generate.sh`，把新证书
  内容更新进客户端代码，重新发布一版客户端——这也是为什么私钥要格外小心
  保管，换一次的成本不小。
