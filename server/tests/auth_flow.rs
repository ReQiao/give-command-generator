//! 集成测试：账号体系全链路。
//!
//! 真起一个服务器子进程、真 TLS 握手、真走 HTTP，验证码从子进程日志里捞
//! （SMS_KIND=log）。这么测的价值在于它同时验证了三件单元测试覆盖不到的事：
//!   1. 请求体 snake_case / 响应体 camelCase 这套字段名约定两边真的对得上；
//!   2. Bearer 鉴权在真实 HTTP 头上工作；
//!   3. "短信挂了就切 SMS_KIND=log 从日志捞码"这条救火通道真的可用，
//!      而不只是文档里的一句承诺。

mod common;

use common::{start_server, ServerOptions};
use serde_json::json;

const PHONE: &str = "13800138000";
const PASSWORD: &str = "Str0ng-Pass-2026";

/// 完整跑一遍：注册 → 收码 → 验证 → 登录 → 改密 → 重置密码。
///
/// 合成一个 test 而不是拆成很多个，是因为每个 test 都要起一个服务器子进程
/// （约一两秒），拆开会让整个测试套件慢好几倍；而这条链路本来就是有先后依赖的。
#[tokio::test]
async fn full_account_lifecycle() {
    let s = start_server("auth", ServerOptions::default()).await;
    let c = s.client();

    // ---------- 注册第一步 ----------
    let before = s.log_line_count();
    let resp = c
        .post(s.url("/v1/auth/register/begin"))
        .json(&json!({ "username": "小明", "password": PASSWORD, "phone": PHONE }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "register/begin 应该成功");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], true);
    // 响应体必须是 camelCase——这是和客户端的契约
    assert_eq!(body["phoneMasked"], "138****8000", "手机号要打码回显，且字段是 camelCase");
    assert!(body["expiresInSecs"].as_u64().unwrap() > 0);
    assert_eq!(body["logMode"], true);

    let code = s.wait_for_code(before).await;

    // ---------- 验证码错了不该放行 ----------
    let wrong = c
        .post(s.url("/v1/auth/register/verify"))
        .json(&json!({ "phone": PHONE, "code": "000000" }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 400, "错误验证码必须被拒");

    // ---------- 注册第二步 ----------
    let resp = c
        .post(s.url("/v1/auth/register/verify"))
        .json(&json!({ "phone": PHONE, "code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "正确验证码应该完成注册");
    let session: serde_json::Value = resp.json().await.unwrap();
    let token = session["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());
    assert_eq!(session["user"]["username"], "小明");
    assert!(
        session["balance"].as_i64().unwrap() > 0,
        "注册成功应该发一次欢迎余额（且只发这一次）"
    );

    // ---------- 验证码用过即废，不能重放 ----------
    let replay = c
        .post(s.url("/v1/auth/register/verify"))
        .json(&json!({ "phone": PHONE, "code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), 400, "同一个验证码不该能用第二次");

    // ---------- Bearer 鉴权 ----------
    let me = c
        .get(s.url("/v1/auth/me"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);
    let me: serde_json::Value = me.json().await.unwrap();
    assert_eq!(me["user"]["username"], "小明");

    let no_auth = c.get(s.url("/v1/auth/me")).send().await.unwrap();
    assert_eq!(no_auth.status(), 401, "没带 token 就该 401");

    let bad_auth = c.get(s.url("/v1/auth/me")).bearer_auth("胡编的token").send().await.unwrap();
    assert_eq!(bad_auth.status(), 401, "伪造 token 就该 401");

    // ---------- 余额接口也要鉴权 ----------
    assert_eq!(
        c.get(s.url("/v1/balance")).send().await.unwrap().status(),
        401,
        "/v1/balance 改造后必须鉴权——改造前它是拿 query 里的 device_id 就能查的"
    );

    // ---------- 充值接口不能再无鉴权白嫖 ----------
    let free_money = c
        .post(s.url("/v1/topup"))
        .json(&json!({ "coins": 888888 }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        free_money.status(),
        401,
        "/v1/topup 改造前是无鉴权免费发币口，任何人 curl 一句就能拿最大档"
    );

    // ---------- 登录：用用户名 ----------
    let login = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": "小明", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200, "用用户名应该能登录");

    // ---------- 登录：用手机号 ----------
    let login = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": PHONE, "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200, "用手机号也应该能登录");
    let login_body: serde_json::Value = login.json().await.unwrap();
    let token2 = login_body["token"].as_str().unwrap().to_string();

    // ---------- 密码错了 ----------
    let bad = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": "小明", "password": "WrongPassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);

    // ---------- 已登录改密码（不发短信） ----------
    let new_password = "Even-Str0nger-2026";
    let changed = c
        .post(s.url("/v1/auth/password/change"))
        .bearer_auth(&token2)
        .json(&json!({ "old_password": PASSWORD, "new_password": new_password }))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200, "改密码应该成功");

    // 改密必须吊销全部会话
    assert_eq!(
        c.get(s.url("/v1/auth/me")).bearer_auth(&token2).send().await.unwrap().status(),
        401,
        "改密之后旧 token 必须失效——否则密码被别人改了，攻击者的旧会话还能用"
    );

    let relogin = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": "小明", "password": new_password }))
        .send()
        .await
        .unwrap();
    assert_eq!(relogin.status(), 200, "新密码应该能登录");

    // ---------- 找回密码 ----------
    let before = s.log_line_count();
    let reset = c
        .post(s.url("/v1/auth/reset/begin"))
        .json(&json!({ "phone": PHONE }))
        .send()
        .await
        .unwrap();
    assert_eq!(reset.status(), 200);
    let reset_code = s.wait_for_code(before).await;

    let final_password = "Reset-Pass-2026!";
    let confirmed = c
        .post(s.url("/v1/auth/reset/confirm"))
        .json(&json!({ "phone": PHONE, "code": reset_code, "new_password": final_password }))
        .send()
        .await
        .unwrap();
    assert_eq!(confirmed.status(), 200, "重置密码应该成功");

    let final_login = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": PHONE, "password": final_password }))
        .send()
        .await
        .unwrap();
    assert_eq!(final_login.status(), 200, "重置后的新密码应该能登录");

    // ---------- 登出 ----------
    let t: serde_json::Value = final_login.json().await.unwrap();
    let t = t["token"].as_str().unwrap();
    assert_eq!(c.post(s.url("/v1/auth/logout")).bearer_auth(t).send().await.unwrap().status(), 200);
    assert_eq!(
        c.get(s.url("/v1/auth/me")).bearer_auth(t).send().await.unwrap().status(),
        401,
        "登出之后 token 就该作废"
    );
}

/// 注册接口不能变成"这个手机号/用户名存不存在"的探针。
#[tokio::test]
async fn registration_does_not_leak_account_existence() {
    let s = start_server("enum", ServerOptions::default()).await;
    let c = s.client();

    // 先注册一个
    let before = s.log_line_count();
    c.post(s.url("/v1/auth/register/begin"))
        .json(&json!({ "username": "已存在的人", "password": PASSWORD, "phone": "13900139000" }))
        .send()
        .await
        .unwrap();
    let code = s.wait_for_code(before).await;
    c.post(s.url("/v1/auth/register/verify"))
        .json(&json!({ "phone": "13900139000", "code": code }))
        .send()
        .await
        .unwrap();

    // 再拿同一个手机号去 begin：响应必须和"全新手机号"长得一模一样
    let dup = c
        .post(s.url("/v1/auth/register/begin"))
        .json(&json!({ "username": "另一个名字", "password": PASSWORD, "phone": "13900139000" }))
        .send()
        .await
        .unwrap();
    assert_eq!(dup.status(), 200, "已注册手机号不该返回 409/400，否则就是个存在性探针");
    let dup_body: serde_json::Value = dup.json().await.unwrap();
    assert_eq!(dup_body["ok"], true);
    assert_eq!(dup_body["phoneMasked"], "139****9000");

    // 登录不存在的账号，和密码错误应该给同样的状态码和文案
    let ghost = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": "根本没这个人", "password": PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(ghost.status(), 401);
    let ghost_text = ghost.text().await.unwrap();

    let wrong_pw = c
        .post(s.url("/v1/auth/login"))
        .json(&json!({ "account": "已存在的人", "password": "definitely-wrong-9" }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_pw.status(), 401);
    assert_eq!(
        ghost_text,
        wrong_pw.text().await.unwrap(),
        "「账号不存在」和「密码错误」必须给一模一样的回复，否则可以枚举账号"
    );
}

/// 输入校验：弱密码、非法手机号、非法用户名都要在发短信之前挡住
/// （挡不住就等于让攻击者拿我们的短信配额当免费轰炸机）。
#[tokio::test]
async fn input_validation_happens_before_sending_sms() {
    let s = start_server("validate", ServerOptions::default()).await;
    let c = s.client();
    let before = s.log_line_count();

    let cases = [
        (json!({ "username": "ok名字", "password": "12345678", "phone": PHONE }), "弱密码"),
        (json!({ "username": "ok名字", "password": PASSWORD, "phone": "12345" }), "非法手机号"),
        (json!({ "username": "x", "password": PASSWORD, "phone": PHONE }), "用户名太短"),
        (json!({ "username": "有 空格", "password": PASSWORD, "phone": PHONE }), "用户名有空格"),
    ];
    for (body, why) in cases {
        let resp = c.post(s.url("/v1/auth/register/begin")).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 400, "{why} 应该被 400 挡住");
    }

    // 关键：上面四次都不该产生任何一条验证码
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let new_lines: Vec<String> = s.captured_logs().into_iter().skip(before).collect();
    assert!(
        !new_lines.iter().any(|l| l.contains("验证码是")),
        "参数校验不通过时绝不能发短信，否则短信配额会被当免费轰炸机刷光。日志：\n{}",
        new_lines.join("\n")
    );
}

/// 同一手机号 60 秒内只能发一条。
#[tokio::test]
async fn sms_rate_limit_blocks_rapid_resend() {
    // 这个用例专门验证冷却限流本身，所以要用真实的非零间隔
    let s = start_server(
        "ratelimit",
        ServerOptions { sms_min_interval_secs: "60", ..Default::default() },
    )
    .await;
    let c = s.client();

    let body = json!({ "username": "限流测试", "password": PASSWORD, "phone": "13700137000" });
    let first = c.post(s.url("/v1/auth/register/begin")).json(&body).send().await.unwrap();
    assert_eq!(first.status(), 200);

    let second = c.post(s.url("/v1/auth/register/begin")).json(&body).send().await.unwrap();
    assert_eq!(second.status(), 429, "60 秒内第二次请求验证码应该被限流");
}

/// admin 路由在没设 ADMIN_TOKEN 时**根本不存在**（404 而不是 403）——
/// 攻击者连"这个路径有没有"都探不出来。
#[tokio::test]
async fn admin_routes_are_absent_without_token() {
    let s = start_server("noadmin", ServerOptions::default()).await;
    let resp = s.client().get(s.url("/v1/admin/stats")).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn admin_routes_require_the_token() {
    let s = start_server(
        "admin",
        ServerOptions { admin_token: Some("test-admin-token"), ..Default::default() },
    )
    .await;
    let c = s.client();

    assert_eq!(
        c.get(s.url("/v1/admin/stats")).send().await.unwrap().status(),
        404,
        "token 不对时也返回 404，不返回 403——不泄露路由存在性"
    );

    let ok = c
        .get(s.url("/v1/admin/stats"))
        .bearer_auth("test-admin-token")
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    let stats: serde_json::Value = ok.json().await.unwrap();
    assert_eq!(stats["smsKind"], "log");
    assert_eq!(stats["lastPersistOk"], true);
}

/// 未登录时 AI 生成必须被挡住——这是整个账号体系存在的直接目的之一。
#[tokio::test]
async fn ai_generate_requires_login() {
    let s = start_server("ai", ServerOptions::default()).await;
    let resp = s
        .client()
        .post(s.url("/v1/ai/generate"))
        .json(&json!({
            "system_prompt": "x",
            "user_text": "给我一把剑",
            "version": "java_1_21_11_plus",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

/// `/v1/version` 是无鉴权的——客户端要靠它判断"服务端还认不认这个版本"，
/// 以及 authRequired 这个逃生开关。
#[tokio::test]
async fn version_endpoint_is_public() {
    let s = start_server("version", ServerOptions::default()).await;
    let resp = s.client().get(s.url("/v1/version")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["authRequired"], true);
}
