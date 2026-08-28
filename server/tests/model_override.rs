//! 验证 /v1/ai/generate 的 model 字段：客户端传了就该原样转发给上游，
//! 不传（或传空白）就该退回 .env 里 AI_MODEL 配置的默认值。
//!
//! 这条测试在账号体系上线后改造过一次：`/v1/ai/generate` 从"请求体带 device_id
//! 就能调"变成了"必须带 Bearer token"，所以现在每个用例都要先真的注册一个账号。

mod common;

use common::{start_server, ServerOptions};
use std::io::{Read, Write};
use std::net::TcpListener;

/// mock 上游：把收到的请求体里的 "model" 字段原样回显在响应 content 里，
/// 这样测试能断言服务器到底转发了哪个模型名，而不用真的连大模型。
///
/// content 必须是 parse_ai_content 认得的 `{intents, explanation}` 形状——
/// 服务器会真的解析这段内容再走 dispatch（不再透传原始 content 给客户端），
/// 所以把回显的模型名塞进 explanation 里，断言时看响应体的 explanation 字段。
fn spawn_model_echo_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]);
            let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(text.len());
            let body: serde_json::Value =
                serde_json::from_str(&text[body_start..]).unwrap_or(serde_json::json!({}));
            let requested_model =
                body.get("model").and_then(|v| v.as_str()).unwrap_or("MISSING").to_string();

            let content = serde_json::json!({
                "intents": [],
                "explanation": format!("echoed_model:{requested_model}")
            })
            .to_string();
            let resp_body = serde_json::json!({
                "choices": [{"message": {"content": content}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    port
}

#[tokio::test]
async fn client_supplied_model_overrides_env_default() {
    let upstream = spawn_model_echo_upstream();
    let s = start_server(
        "model",
        ServerOptions {
            upstream_port: Some(upstream),
            ai_model: "qwen-env-default",
            ..Default::default()
        },
    )
    .await;

    let token = s.register_and_login("模型测试", "13600136000", "M0del-Test-2026").await;
    let c = s.client();

    let ask = |model: Option<&'static str>| {
        let c = c.clone();
        let url = s.url("/v1/ai/generate");
        let token = token.clone();
        async move {
            let mut body = serde_json::json!({
                "system_prompt": "sys",
                "user_text": "hi",
                "version": "java_1_21_5",
            });
            if let Some(m) = model {
                body["model"] = serde_json::json!(m);
            }
            let resp: serde_json::Value = c
                .post(url)
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            resp["explanation"].as_str().unwrap_or_default().to_string()
        }
    };

    assert_eq!(
        ask(Some("qwen3.8-max")).await,
        "echoed_model:qwen3.8-max",
        "客户端指定了模型，服务器应该转发这个而不是 .env 默认值"
    );
    assert_eq!(
        ask(None).await,
        "echoed_model:qwen-env-default",
        "没传 model 字段应该退回 .env 默认值"
    );
    // 防的是前端 apiModel.trim() 传空串这种情况
    assert_eq!(
        ask(Some("   ")).await,
        "echoed_model:qwen-env-default",
        "空白字符串的 model 应该等同于没传"
    );
}
