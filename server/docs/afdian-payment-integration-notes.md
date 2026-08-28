# 爱发电（Afdian）支付接入参考资料

存放位置说明：现在 `billing_recharge` 还是"点了预设档位直接免费加余额，没有
接真实支付网关"（见 `src-tauri/src/billing.rs` 注释）。这份文档是为将来接入
爱发电做真实支付网关时留的参考，**现在不动手实现**——爱发电的 OAuth2 关联
授权功能需要先向官方申请认证（应用名称/可信域名/clientSecret），认证完成前
没法真正跑通授权登录这部分；Webhook + API 两条路径本身不需要审核即可用，
认证卡住的主要是 OAuth2 授权登录（如果决定走这条路）。

来源：<https://ifdian.net/p/9c65d9cc617011ed81c352540025c377>
（爱发电开发者后台：<https://ifdian.net/dashboard/dev>）

---

## 三种对接方式

1. **Webhook**——爱发电每次有订单时主动 POST 通知到开发者配置的 URL。需要
   在开发者后台配置回调地址；服务器异常时不保证及时送达，官方建议配合 API
   兜底轮询。
2. **API**——开发者主动请求爱发电查询历史订单/赞助者列表，需要 `user_id` +
   API token（开发者后台可拿到）。
3. **OAuth2 关联授权**——需要向官方申请（应用名称、可信域名、
   clientSecret），走 `authorization_code` 模式，让用户在爱发电侧登录授权，
   换取 `user_id`。**这条路径需要先申请审核，是当前接入的阻塞点。**

对于"充值兑换灵魂币"这个场景，大概率只需要 **Webhook + API**，不需要
OAuth2（不需要绑定爱发电账号身份，只需要知道"谁付了钱、付了多少、对应哪个
兑换码/自定义信息"）——`custom_order_id` 这个字段可以由购买链接的 URL 传参，
可以用来编码"这是给哪个 device_id / 灵魂币档位的订单"。

## Webhook

回调 payload 示例（`data.type` 目前固定为 `"order"`）：

```json
{
  "ec": 200,
  "em": "ok",
  "data": {
    "type": "order",
    "order": {
      "out_trade_no": "2021062321XXX1083454010626",
      "custom_order_id": "Steam12345",
      "user_id": "adf397fe83748XXXcee52540025c377",
      "user_private_id": "fdf981fu8f7g891euaceXXX430321c377",
      "plan_id": "a45353328af91XXX052540025c377",
      "month": 1,
      "total_amount": "5.00",
      "show_amount": "5.00",
      "status": 2,
      "remark": "",
      "redeem_id": "",
      "product_type": 0,
      "discount": "0.00",
      "sku_detail": [
        {
          "sku_id": "b082342c4aba1XXX5cb52540025c377",
          "count": 1,
          "name": "15000 赏金/货币 兑换码",
          "album_id": "",
          "pic": "https://pic1.afdiancdn.com/user/8a8e408a3aeb11eab26352540025c377/common/sfsfsff.jpg"
        }
      ],
      "address_person": "",
      "address_phone": "",
      "address_address": ""
    }
  }
}
```

开发者必须响应固定结构，否则平台判定回调失败（会话内没提到是否会重试，建议
按幂等设计，见下方"接入要点"）：

```json
{"ec":200,"em":""}
```

`status == 2` 才表示交易成功；目前只推送这一种状态。

### 签名校验（Webhook）

公钥（PEM）：

```
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwwdaCg1Bt+UKZKs0R54y
lYnuANma49IpgoOwNmk3a0rhg/PQuhUJ0EOZSowIC44l0K3+fqGns3Ygi4AfmEfS
4EKbdk1ahSxu7Zkp2rHMt+R9GarQFQkwSS/5x1dYiHNVMiR8oIXDgjmvxuNes2Cr
8fw9dEF0xNBKdkKgG2qAawcN1nZrdyaKWtPVT9m2Hl0ddOO9thZmVLFOb9NVzgYf
jEgI+KWX6aY19Ka/ghv/L4t1IXmz9pctablN5S0CRWpJW3Cn0k6zSXgjVdKm4uN7
jRlgSRaf/Ind46vMCm3N2sgwxu/g3bnooW+db0iLo13zzuvyn727Q3UDQ0MmZcEW
MQIDAQAB
-----END PUBLIC KEY-----
```

签名的原文是 `order.out_trade_no + order.user_id + order.plan_id +
order.total_amount` 依次拼接的字符串，用公钥做 RSA-SHA256 验签（`sign` 字段
是 base64）：

```php
// sign_str 为待验证字符串，sign 为回调数据里的 sign 字段
public function verifySign($sign_str, $sign) {
    $publicKey = "上面的公钥";
    $key = openssl_get_publickey($publicKey);
    return openssl_verify($sign_str, base64_decode($sign), $key, 'SHA256');
}
```

Rust 侧等价实现可以用 `rsa` + `sha2` crate（`RsaPublicKey` + PKCS1v15 签名
verify），到时候接入时再评估要不要引入这两个新依赖（当前 `server/Cargo.toml`
是纯 rustls 技术栈，加密相关的 crate 选型要谨慎，参考之前"不引入 C 依赖"
的取舍）。

## API（服务端主动查询）

请求签名规则：

```
sign = md5(token + "params" + params + "ts" + ts + "user_id" + user_id)
```

其中 `token` 只参与签名计算，不出现在请求体里；`params` 是具体接口参数的
JSON 字符串（不是对象，是字符串化后的 JSON）；`ts` 是秒级时间戳，允许
3600 秒内的误差。

测试签名的接口：`https://ifdian.net/api/open/ping`

### 查订单

`https://ifdian.net/api/open/query-order`

- `params.page`：翻页（1/2/3...），按创建时间倒序
- `params.out_trade_no`：按订单号查（逗号分隔多个）
- `params.per_page`：默认 50，支持 1-100

### 查赞助者

`https://ifdian.net/api/open/query-sponsor`

- `params.page`：翻页，默认每页 20，支持 `per_page` 1-100
- `params.user_id`：查指定用户（逗号分隔多个）

### 查方案 / 按订单号查随机自动回复 / 更新方案自动回复 / 发私信

这几个是 2025 年后新增的接口，字段细节见文末原文引用，暂时用不上（灵魂币
充值场景只需要"查订单状态"）。

## 接入要点（等真正做的时候再展开）

1. 用 `custom_order_id` 编码 `device_id` + 目标灵魂币档位（购买链接 URL
   传参），Webhook 收到订单后据此调用 `Ledger::topup`
2. `status == 2` 且验签通过才当作有效充值；验签失败或字段缺失应该拒绝
   （返回非 200 的 `ec`，或者干脆记日志但不落账，绝不能"验签失败也照样加币"）
3. 幂等：`out_trade_no` 全局唯一，落账前检查这个订单号是否已经处理过，
   防止 Webhook 重复投递导致重复加币（`Ledger` 现在的激活码兑换已经有类似
   的"已使用过的码"去重逻辑，可以参考同样的模式）
4. Webhook 送达不保证，建议加一个定时任务用 `query-order` API 轮询兜底
   （比如每小时拉一次最近订单，和本地"已处理"记录对账，捞漏单）
5. 需要先在 <https://ifdian.net/dashboard/dev> 拿到 `user_id` 和 API token，
   这两个和 API 私钥一样需要只存在服务器环境变量里，不能进客户端/仓库
