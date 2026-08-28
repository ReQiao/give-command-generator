//! 集成测试：真的起一个 server/ 服务器子进程，真的用证书锁定连过去，
//! 调用的是 lib.rs 里真实的 tauri::command 函数本体（billing_state /
//! billing_activate / billing_recharge / ai_generate），不是重新拼一遍
//! 逻辑去测——这样才能真正验证"客户端这边接得对不对"，而不只是
//! "remote.rs 这几个函数写得对不对"。
//!
//! 用 SOUL_LANTERN_PINNED_CERT_FILE 环境变量把 remote.rs 的证书锁定指向
//! 这里现场生成的临时证书，不碰打包进正式客户端的那个占位常量。

use soul_lantern_lib::{ai, billing, device};
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

fn server_binary() -> PathBuf {
    // 复用仓库根目录 server/ 已经编译好的二进制，不在这里重新编译一遍
    // （server 是独立 crate，不是 src-tauri 的依赖，没法用 CARGO_BIN_EXE_ 拿到）。
    let candidates = [
        "../server/target/debug/soul-lantern-server",
        "../server/target/x86_64-unknown-linux-musl/release/soul-lantern-server",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return p;
        }
    }
    panic!(
        "找不到 server 的编译产物，先在 server/ 目录下跑一次 `cargo build`\n\
         尝试过的路径：{candidates:?}"
    );
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

fn spawn_mock_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        // 循环 accept，因为一次测试里可能触发不止一次 AI 调用
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            // content 必须是服务器 give::parse::parse_ai_content 认得的
            // {intents, explanation} 形状——服务器现在会真的解析这段内容再走
            // dispatch（不再透传原始 content 给客户端）。一条 say 意图足以
            // 验证全链路（含证书锁定握手 + 服务器端 dispatch + 扣费）。
            let body = r#"{"choices":[{"message":{"content":"{\"intents\":[{\"command\":\"say\",\"form\":{\"message\":\"hi\"}}],\"explanation\":\"ok\"}"}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    port
}

/// 这个测试端口范围避开 server/tests/tls_pinning.rs 用的那两段，两个 crate
/// 的测试有可能在 CI 里并行跑，撞端口会导致偶发失败。
fn start_test_server() -> (Guard, PathBuf, u16) {
    let dir = std::env::temp_dir().join(format!("soul-client-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (crt, key) = generate_cert(&dir, "127.0.0.1");
    let upstream_port = spawn_mock_upstream();
    let bind_port = 20000 + (std::process::id() % 1000) as u16;

    let child = Command::new(server_binary())
        .env("TLS_CERT", &crt)
        .env("TLS_KEY", &key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{bind_port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen3.7-plus")
        .env("AI_API_KEY", "test-key-not-real")
        .spawn()
        .expect("启动服务器子进程失败");

    (Guard(child), crt, bind_port)
}

/// 注意：remote.rs 的 HTTPS 客户端是进程级单例（OnceLock），一旦第一次用
/// 某个 SOUL_LANTERN_SERVER_BASE / 证书初始化，同进程里后续调用都复用同一
/// 个实例，环境变量之后再改也不会生效。所以这个文件只用一个测试函数覆盖
/// 全部场景，而不是拆成多个 #[tokio::test]——拆开会因为初始化时机不确定
/// 互相踩踏，不如老老实实一个测试里按顺序走完，调用真实的
/// billing::billing_state / billing_activate / billing_recharge /
/// ai::ai_generate（就是 lib.rs 里注册给前端调用的那几个 tauri::command
/// 函数本体），而不是绕过它们直接调 remote:: 里的底层函数。
#[tokio::test]
async fn full_client_flow_against_real_server() {
    let (_guard, crt, bind_port) = start_test_server();
    // Rust 2024 里改环境变量是 unsafe（进程全局可变状态，别的线程同时读会有
    // 数据竞争）。这个文件只有一个测试函数、不会并行跑，这里是安全的。
    unsafe {
        std::env::set_var("SOUL_LANTERN_PINNED_CERT_FILE", &crt);
        std::env::set_var("SOUL_LANTERN_SERVER_BASE", format!("https://127.0.0.1:{bind_port}"));
    }
    tokio::time::sleep(Duration::from_millis(800)).await; // 等服务器起来监听端口

    // device::get_or_create 在测试环境里也能正常工作：定位不到配置目录时
    // 退化成每次生成新 id，不 panic；这里只确认同一次运行里稳定不变。
    let id_a = device::get_or_create();
    let id_b = device::get_or_create();
    assert_eq!(id_a, id_b, "同一次运行里应该拿到同一个 device_id");

    // billing_topup_tiers 是纯本地静态数据，不联网
    assert_eq!(billing::billing_topup_tiers().len(), 8);

    // 查余额：新设备应该拿到首次体验额度（9999，跟 server/src/ledger.rs 的
    // WELCOME_BALANCE 保持一致）
    let state = billing::billing_state().await.expect("首次查余额不该失败");
    assert_eq!(state.balance, 9999);
    assert!(!state.activated);

    // 充值：只接受预设档位
    assert!(billing::billing_recharge(999).await.is_err(), "非预设档位应该被服务器拒绝");
    let after_topup = billing::billing_recharge(1000).await.expect("预设档位充值应该成功");
    assert_eq!(after_topup.balance, 9999 + 1000);

    // 激活码格式错误：本地就该直接拦下，不用等服务器
    assert!(billing::billing_activate("NOPE".to_string()).await.is_err());

    // 激活码兑换：应该叠加而不是覆盖已有余额
    let activated = billing::billing_activate("SOUL-AB12-CD34-EF56".to_string())
        .await
        .expect("格式正确且没兑过的激活码应该兑换成功");
    assert_eq!(activated.balance, 9999 + 1000 + 100);
    assert!(activated.activated);

    // 同一个码不能再兑一次（哪怕还是同一个 device_id）
    assert!(billing::billing_activate("SOUL-AB12-CD34-EF56".to_string()).await.is_err());

    // AI 生成：走真实 HTTPS 请求到测试服务器，服务器再转发给本地 mock 上游，
    // 全链路验证——包括证书锁定握手、请求/响应的 JSON 结构对得上。
    let ai_resp = ai::ai_generate(
        "system prompt".to_string(),
        "做一把弓".to_string(),
        None,
        "java_1_21_5".to_string(),
        None,
    )
    .await
    .expect("ai_generate 本身不应该返回 Err（失败信息走 AiResponse.error 字段）");
    assert!(ai_resp.ok, "mock 上游应该让这次调用成功：{:?}", ai_resp.error);
    assert_eq!(ai_resp.commands, vec!["say hi".to_string()], "服务器应该已经把意图分派构建成命令字符串");
    assert!(ai_resp.balance.is_some(), "成功调用后余额应该是具体数字而不是 None");
    // mock 上游返回 usage 15 token，qwen3.7-plus 保底扣费不会是 0
    assert!(ai_resp.balance.unwrap() < activated.balance, "成功调用应该扣了费");
}
