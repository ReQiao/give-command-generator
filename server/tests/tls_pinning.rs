//! 集成测试：证书锁定这条链路要用真实 TLS 握手验证，不能只测「代码逻辑」。
//!
//! 背景：这条链路真实踩过一次坑——`openssl req -x509` 默认给自签名证书打上
//! `CA:TRUE`，`curl --cacert` 校验完全不介意这个，看起来一切正常；但客户端
//! 实际用的 rustls 校验器严格得多，会直接拒绝「一张标了 CA:TRUE 的证书被
//! 服务器当自己的叶子证书用」，报错 `CaUsedAsEndEntity`。这个坑只有真的用
//! rustls 发一次请求才能发现，单元测试无论怎么检查证书字段的字符串内容都测
//! 不出来（问题是 rustls 校验器的语义，不是数据格式）。所以这里老老实实起
//! 一个真实的服务器子进程、生成一张真实证书、真的握手一次。
//!
//! 覆盖两个方向：
//!   1. 正例：客户端用同一张证书做锁定，应该连得上（这是回归防线，防的就是
//!      上面说的 CaUsedAsEndEntity）。
//!   2. 反例：换一张不相关的自签名证书去连，应该被拒绝——不然"锁定"是假的，
//!      变成"随便一张自签名证书都能连"，起不到防中间人的作用。

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

/// 生成一张自签名证书，扩展字段和 certs/generate.sh 保持一致——两边分开维护
/// 是因为一个是部署脚本、一个是测试代码，但逻辑必须同步，改一边记得改另一边。
///
/// `name` 必须在同一个 `dir` 里保持唯一——早前这里两个证书都硬编码用
/// `test.crt`/`test.key`，导致反例测试里"生成第二张不相关的证书"这一步
/// 直接把第一张的文件覆盖掉了，测试看起来是在验证"锁定错误证书应该被拒绝"，
/// 实际上服务器和客户端用的是同一份文件内容，白测了一场。
fn generate_cert(dir: &PathBuf, name: &str, cn: &str) -> (PathBuf, PathBuf) {
    let key = dir.join(format!("{name}.key"));
    let crt = dir.join(format!("{name}.crt"));
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
        .expect("需要本机装了 openssl 才能跑这个集成测试");
    assert!(status.success(), "openssl 生成证书失败");
    (crt, key)
}

/// 起一个最小的 mock 上游：不需要真的调大模型，只要能让 /v1/ai/generate
/// 走完整条链路即可，这个测试的重点是 TLS 握手，不是计费逻辑（那部分
/// src/ai_proxy.rs 和 src/ledger.rs 里的单测已经覆盖）。
fn spawn_mock_upstream() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = r#"{"choices":[{"message":{"content":"{}"}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (port, handle)
}

fn pinned_client(cert_path: &PathBuf) -> reqwest::Client {
    let cert_pem = std::fs::read(cert_path).unwrap();
    let cert = reqwest::Certificate::from_pem(&cert_pem).unwrap();
    reqwest::Client::builder()
        .tls_built_in_root_certs(false) // 关键：只信任下面这一张，不信任公共 CA
        .add_root_certificate(cert)
        .build()
        .unwrap()
}

#[tokio::test]
async fn pinned_client_connects_to_matching_self_signed_cert() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let dir = std::env::temp_dir().join(format!("soul-tls-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (crt, key) = generate_cert(&dir, "server", "127.0.0.1");
    let (upstream_port, _upstream) = spawn_mock_upstream();

    let bind_port = 18000 + (std::process::id() % 1000) as u16; // 避免和并行测试撞端口
    let child = Command::new(env!("CARGO_BIN_EXE_soul-lantern-server"))
        .env("TLS_CERT", &crt)
        .env("TLS_KEY", &key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{bind_port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen3.7-plus")
        .env("AI_API_KEY", "test-key-not-real")
        .spawn()
        .expect("启动服务器子进程失败");
    let _guard = Guard(child);
    tokio::time::sleep(Duration::from_millis(800)).await; // 等服务器起来监听端口

    let client = pinned_client(&crt);
    let resp = client
        .get(format!("https://127.0.0.1:{bind_port}/v1/health"))
        .send()
        .await
        .expect("用同一张证书锁定，应该能连上——如果这里失败，多半是证书的 \
                 basicConstraints/keyUsage 扩展字段又被写成 CA 证书的样子了 \
                 （CaUsedAsEndEntity 那个坑）");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn pinned_client_rejects_a_different_self_signed_cert() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let dir = std::env::temp_dir().join(format!("soul-tls-test-mismatch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (server_crt, server_key) = generate_cert(&dir, "server", "127.0.0.1");
    // 客户端锁定的是另外一张证书——不是服务器实际在用的那张，文件名必须不同
    // （见 generate_cert 上的注释，这条测试早前就是因为文件名撞了才白测一场）
    let (unrelated_crt, _unrelated_key) = generate_cert(&dir, "unrelated", "127.0.0.1");
    let (upstream_port, _upstream) = spawn_mock_upstream();

    let bind_port = 19000 + (std::process::id() % 1000) as u16;
    let child = Command::new(env!("CARGO_BIN_EXE_soul-lantern-server"))
        .env("TLS_CERT", &server_crt)
        .env("TLS_KEY", &server_key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{bind_port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen3.7-plus")
        .env("AI_API_KEY", "test-key-not-real")
        .spawn()
        .expect("启动服务器子进程失败");
    let _guard = Guard(child);
    tokio::time::sleep(Duration::from_millis(800)).await;

    // 客户端锁定的是 unrelated_crt，不是服务器真正在用的 server_crt
    let client = pinned_client(&unrelated_crt);
    let result = client.get(format!("https://127.0.0.1:{bind_port}/v1/health")).send().await;
    assert!(result.is_err(), "锁定了不匹配的证书还能连上，说明证书锁定形同虚设");
}
