//! 验证 /v1/ai/generate 的 model 字段：客户端传了就该原样转发给上游，
//! 不传就该退回 .env 里 AI_MODEL 配置的默认值。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn generate_cert(dir: &PathBuf, cn: &str) -> (PathBuf, PathBuf) {
    let key = dir.join("test.key");
    let crt = dir.join("test.crt");
    let status = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1",
            "-keyout", key.to_str().unwrap(), "-out", crt.to_str().unwrap(),
            "-days", "1", "-nodes",
            "-subj", &format!("/CN={cn}"),
            "-addext", &format!("subjectAltName=IP:{cn}"),
            "-addext", "basicConstraints=critical,CA:FALSE",
            "-addext", "keyUsage=critical,digitalSignature,keyEncipherment",
            "-addext", "extendedKeyUsage=serverAuth",
        ])
        .status()
        .expect("需要本机装了 openssl");
    assert!(status.success());
    (crt, key)
}

/// mock 上游：把收到的请求体里的 "model" 字段原样回显在响应 content 里，
/// 这样测试能断言服务器到底转发了哪个模型名，而不用真的连大模型。
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
            let requested_model = body.get("model").and_then(|v| v.as_str()).unwrap_or("MISSING").to_string();

            let content = serde_json::json!({ "echoed_model": requested_model }).to_string();
            let resp_body = serde_json::json!({
                "choices": [{"message": {"content": content}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                resp_body.len(), resp_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    port
}

fn pinned_client(cert_path: &PathBuf) -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cert_pem = std::fs::read(cert_path).unwrap();
    let cert = reqwest::Certificate::from_pem(&cert_pem).unwrap();
    reqwest::Client::builder().tls_built_in_root_certs(false).add_root_certificate(cert).build().unwrap()
}

#[tokio::test]
async fn client_supplied_model_overrides_env_default() {
    let dir = std::env::temp_dir().join(format!("soul-model-override-{}-qwen-env-default", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (crt, key) = generate_cert(&dir, "127.0.0.1");
    let upstream_port = spawn_model_echo_upstream();
    let bind_port = 22000 + (std::process::id() % 1000) as u16;

    let child = Command::new(env!("CARGO_BIN_EXE_soul-lantern-server"))
        .env("TLS_CERT", &crt)
        .env("TLS_KEY", &key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{bind_port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen-env-default")
        .env("AI_API_KEY", "test-key-not-real")
        .spawn()
        .expect("启动服务器子进程失败");
    let _guard = Guard(child);
    tokio::time::sleep(Duration::from_millis(800)).await;

    let client = pinned_client(&crt);
    let base = format!("https://127.0.0.1:{bind_port}");

    // 客户端明确指定了模型：服务器应该原样转发这一个，不是 .env 默认值
    let resp: serde_json::Value = client
        .post(format!("{base}/v1/ai/generate"))
        .json(&serde_json::json!({
            "device_id": "dev-model-a",
            "system_prompt": "sys",
            "user_text": "hi",
            "model": "qwen3.8-max",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let content: serde_json::Value = serde_json::from_str(resp["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["echoed_model"], "qwen3.8-max", "客户端指定了模型，服务器应该转发这个而不是 .env 默认值");

    // 客户端没传 model 字段：应该退回 .env 里的默认值
    let resp: serde_json::Value = client
        .post(format!("{base}/v1/ai/generate"))
        .json(&serde_json::json!({
            "device_id": "dev-model-b",
            "system_prompt": "sys",
            "user_text": "hi",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let content: serde_json::Value = serde_json::from_str(resp["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["echoed_model"], "qwen-env-default", "没传 model 字段应该退回 .env 默认值");

    // 客户端传了空字符串：视同没传，同样退回默认值（防的是前端 apiModel.trim() 传空串这种情况）
    let resp: serde_json::Value = client
        .post(format!("{base}/v1/ai/generate"))
        .json(&serde_json::json!({
            "device_id": "dev-model-c",
            "system_prompt": "sys",
            "user_text": "hi",
            "model": "   ",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let content: serde_json::Value = serde_json::from_str(resp["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["echoed_model"], "qwen-env-default", "空白字符串的 model 应该等同于没传");
}
