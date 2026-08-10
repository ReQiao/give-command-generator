# 部署清单：一步步在你的阿里云 ECS 上跑起来

全程你自己在自己的终端里操作（SSH 连你的服务器），密码/私钥全程不经过我。
我会把编译好的二进制文件发给你，其余步骤都是复制粘贴命令。

## 第 0 步：把二进制传到服务器

我会把 `soul-lantern-server`（已经编译好、静态链接，不需要服务器上装 Rust）
发给你。在**你自己电脑的终端**里，把这个文件传到服务器（把路径换成你实际
下载到的位置）：

```bash
scp soul-lantern-server root@120.26.175.121:/tmp/
```

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

```bash
cd /opt/soul-lantern
# 把 generate.sh 的内容粘贴保存到这里之后：
chmod +x generate.sh
./generate.sh 120.26.175.121
```

跑完这一步，`/opt/soul-lantern/` 下会多出 `server.crt`（公钥，等下要用）和
`server.key`（私钥，往后就一直留在这台服务器上，不要复制到别处）。

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

同样，把仓库里 `server/deploy/soul-lantern.service` 的内容复制粘贴保存成
`/etc/systemd/system/soul-lantern.service`，然后：

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

回到**你自己的电脑**，把服务器上的 `server.crt` 也复制一份到本地（内容不是
秘密，之前已经贴给我了，你自己留一份也行），然后：

```bash
curl --cacert server.crt https://120.26.175.121:8443/v1/health
```

看到返回 `ok` 就说明整条链路（证书、防火墙、安全组、服务本身）全部打通了。

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
