# 灵魂灯笼服务端

账本（余额 / 激活码）+ AI 调用代理。存在的理由：客户端此前把大模型 key 直接
编译进分发出去的软件包里，被人拆出来盗刷过一次真实发生的事故。这个服务把
"调用大模型"这一步挪到这里——key 只活在这台服务器的环境变量里，客户端只
发"我要生成什么"，从来碰不到能直接花钱的凭证。顺带把余额/激活码也收进来，
本地文件签名校验再怎么做也只能防"随手改", 真正的权威数据只能放用户碰不到
的地方。

## 部署环境

国内机房裸 IP，没有域名/备案，所以走**自签名证书 + 客户端证书锁定**，不是
公共 CA 签发的证书——客户端只信任这一张证书，不信任系统公共信任链，这样
即便没有域名也能有真正意义上的加密和身份校验（防中间人）。

## 目录

- `src/main.rs` —— HTTP 路由 + TLS 启动
- `src/ledger.rs` —— 余额/激活码账本，本地 JSON 文件持久化
- `src/ai_proxy.rs` —— 价目表 + 调用上游大模型
- `certs/generate.sh` —— 生成自签名证书的脚本，`./generate.sh <公网IP>`
- `certs/server.crt` —— 当前生效证书的公钥部分（不是秘密，随客户端分发）
- `certs/server.key` —— **私钥，被 .gitignore 挡住，绝不能进仓库**

## 环境变量

| 变量 | 说明 |
|---|---|
| `AI_ENDPOINT` | 上游大模型接口地址（如阿里云百炼的 chat/completions 完整路径） |
| `AI_MODEL` | 模型名 |
| `AI_API_KEY` | 真实 API key，**只在服务器上设置，不进任何代码/仓库** |
| `TLS_CERT` | 证书文件路径，默认 `certs/server.crt` |
| `TLS_KEY` | 私钥文件路径，默认 `certs/server.key` |
| `LEDGER_PATH` | 账本 JSON 文件路径，默认 `ledger.json` |
| `BIND_ADDR` | 监听地址，默认 `0.0.0.0:8443` |

## 本地跑起来

```bash
export AI_ENDPOINT=... AI_MODEL=... AI_API_KEY=...
cargo run
```

## 接口

全部 JSON，路径前缀 `/v1`：

- `GET /v1/health` —— 存活检查
- `GET /v1/balance?device_id=...` —— 查余额，设备第一次出现会自动发首次体验额度
- `POST /v1/activate {device_id, license_key}` —— 激活码兑换，全局唯一（同一个码不能被两个设备各兑一次）
- `POST /v1/topup {device_id, coins}` —— 充值，只接受预设档位
- `POST /v1/ai/generate {device_id, system_prompt, user_text, history?}` —— AI 调用代理，成功按真实 token 用量扣费
