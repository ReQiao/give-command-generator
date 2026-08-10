#!/usr/bin/env bash
# 生成绑定服务器公网 IP 的自签名证书（走裸 IP，不走域名/公共 CA）。
#
# 用法：
#   ./generate.sh 120.26.175.121
#
# 产出：
#   server.key —— 私钥，只能留在服务器上，绝不能进 git 仓库、绝不能发给任何人
#   server.crt —— 公钥证书，这个要被编进客户端代码里做"证书锁定"
#
# 有效期给了 20 年，因为这张证书没有 Let's Encrypt 那种自动续期机制——
# 换证书意味着要重新编译发布一版客户端（公钥变了，锁定的目标就变了），
# 不是随便就能做的事，所以选择"基本不用管"而不是"三个月过期一次"。
# 代价是：私钥一旦泄露，有效期长这件事本身没有任何好处、只有坏处——
# 一定要保证 server.key 的文件权限是 600、只有 root 能读。

set -euo pipefail

IP="${1:?用法: ./generate.sh <服务器公网IP>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout server.key -out server.crt \
  -days 7300 -nodes \
  -subj "/CN=${IP}" \
  -addext "subjectAltName=IP:${IP}"

chmod 600 server.key
chmod 644 server.crt

echo "生成完成："
echo "  server.key（私钥，权限已设为 600，只能留在服务器上）"
echo "  server.crt（公钥证书，需要复制内容编进客户端代码做证书锁定）"
echo
openssl x509 -in server.crt -noout -text | grep -A1 "Subject Alternative Name"
