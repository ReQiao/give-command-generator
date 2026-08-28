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

mod common;

use common::{generate_cert, pinned_client, spawn_mock_upstream, temp_dir, Guard};
use std::process::Command;
use std::time::{Duration, Instant};

/// 起一个服务器并等它真的能响应（不用固定 sleep——加了用户表/备份/写权限自检之后
/// 启动变慢了，固定 800ms 会变成随机失败的 flaky 测试）。
async fn start_with_certs(crt: &std::path::Path, key: &std::path::Path, ledger: &std::path::Path) -> (u16, Guard) {
    let upstream_port = spawn_mock_upstream();
    let port = common::free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_soul-lantern-server"))
        .env("TLS_CERT", crt)
        .env("TLS_KEY", key)
        .env("LEDGER_PATH", ledger)
        .env("BIND_ADDR", format!("127.0.0.1:{port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen3.7-plus")
        .env("AI_API_KEY", "test-key-not-real")
        .env("AUTH_PEPPER", "tls-test-pepper")
        .spawn()
        .expect("启动服务器子进程失败");
    (port, Guard(child))
}

async fn wait_ready(client: &reqwest::Client, port: u16) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if client.get(format!("https://127.0.0.1:{port}/v1/health")).send().await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn pinned_client_connects_to_matching_self_signed_cert() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let dir = temp_dir("tls-match");
    let (crt, key) = generate_cert(&dir, "server", "127.0.0.1");
    let (port, _guard) = start_with_certs(&crt, &key, &dir.join("ledger.json")).await;

    let client = pinned_client(&crt);
    assert!(
        wait_ready(&client, port).await,
        "用同一张证书锁定，应该能连上——如果这里失败，多半是证书的 \
         basicConstraints/keyUsage 扩展字段又被写成 CA 证书的样子了（CaUsedAsEndEntity 那个坑），\
         或者是 axum-server 的 feature 被改回了会引入 aws-lc 的 tls-rustls"
    );

    let resp = client.get(format!("https://127.0.0.1:{port}/v1/health")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn pinned_client_rejects_a_different_self_signed_cert() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let dir = temp_dir("tls-mismatch");
    let (server_crt, server_key) = generate_cert(&dir, "server", "127.0.0.1");
    // 客户端锁定的是另外一张证书——不是服务器实际在用的那张，文件名必须不同
    // （见 common::generate_cert 上的注释，这条测试早前就是因为文件名撞了才白测一场）
    let (unrelated_crt, _unrelated_key) = generate_cert(&dir, "unrelated", "127.0.0.1");
    let (port, _guard) = start_with_certs(&server_crt, &server_key, &dir.join("ledger.json")).await;

    // 先用正确证书确认服务器确实起来了，否则下面的"连不上"可能只是因为还没启动
    assert!(wait_ready(&pinned_client(&server_crt), port).await, "服务器没起来，这条测试无从谈起");

    let client = pinned_client(&unrelated_crt);
    let result = client.get(format!("https://127.0.0.1:{port}/v1/health")).send().await;
    assert!(result.is_err(), "锁定了不匹配的证书还能连上，说明证书锁定形同虚设");
}
