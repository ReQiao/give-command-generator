//! 集成测试：真的起一个服务端子进程，真的用证书锁定连过去，
//! 调用的是 lib.rs 里真实的 tauri::command 函数本体（auth_* / billing_* /
//! ai_generate），不是重新拼一遍逻辑去测——这样才能真正验证"客户端这边接得
//! 对不对"，而不只是"remote.rs 这几个函数写得对不对"。
//!
//! 这个文件真正的价值在于**它是客户端和服务端之间字段名契约的唯一守门人**：
//! 请求体 snake_case、响应体 camelCase 这套约定，两边各自的单测都是绿的，
//! 只有真实 HTTP 往返才抓得到对不上。这次账号体系一口气加了 9 组新
//! 请求/响应 struct，是这个坑历史上最容易复发的一刻。
//!
//! 用 SOUL_LANTERN_PINNED_CERT_FILE 环境变量把 remote.rs 的证书锁定指向
//! 这里现场生成的临时证书，不碰打包进正式客户端的那个占位常量。
//!
//! **服务端已经拆到独立的私有仓库**，所以这个测试默认跑不起来——编译好服务端
//! 之后用 `SOUL_LANTERN_SERVER_BIN=<二进制路径> cargo test` 指过来。找不到就
//! 自动跳过（见 `server_binary()` 上的注释）。

use soul_lantern_lib::{ai, auth, billing};
use std::io::{BufRead, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PEPPER: &str = "client-integration-test-pepper";

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 找服务端二进制。**找不到时返回 None，不 panic。**
///
/// 【为什么不能 panic 了】服务端已经从这个仓库里拆出去，放进一个独立的私有仓库。
/// 公开仓库里 `server/` 不存在，谁 clone 下来跑 `cargo test` 都会撞上这个 panic，
/// 而那不是"测试失败"，只是"这台机器上没有服务端"。
///
/// 【但这个测试不能删】它是客户端与服务端之间**字段名契约的唯一守门人**——
/// 请求体 snake_case、响应体 camelCase 这套约定，两边各自的单测都是绿的，
/// 只有真实 HTTP 往返才抓得到对不上。拆成两个仓库之后，两边独立演进、
/// 契约漂移的风险只增不减，所以它比以前更需要存在。
///
/// 于是改成：有服务端就跑（拆仓之后这条路径只有作者本人走得到），
/// 没有就跳过并打一行说明。
fn server_binary() -> Option<PathBuf> {
    // 服务端仓库在哪由环境变量说了算——它现在是独立仓库，路径因人而异。
    if let Ok(p) = std::env::var("SOUL_LANTERN_SERVER_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
        eprintln!("SOUL_LANTERN_SERVER_BIN 指向的文件不存在：{}", p.display());
        return None;
    }
    // 兜底猜几个常见位置：服务端仓库和客户端仓库并排放着的情况。
    const CANDIDATES: [&str; 4] = [
        "../../soul-lantern-server/target/debug/soul-lantern-server",
        "../../soul-lantern-server/target/release/soul-lantern-server",
        // 拆仓之前的老位置，本地还留着 server/ 的话仍然能用
        "../server/target/debug/soul-lantern-server",
        "../server/target/release/soul-lantern-server",
    ];
    CANDIDATES.iter().map(PathBuf::from).find(|p| p.exists())
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
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            // content 必须是服务器 give::parse::parse_ai_content 认得的
            // {intents, explanation} 形状——服务器会真的解析这段内容再走
            // dispatch。一条 say 意图足以验证全链路。
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

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// SMS_KIND=log 时验证码只出现在服务器日志里，这里把子进程输出接过来。
type Logs = Arc<Mutex<Vec<String>>>;

fn start_test_server(bin: &PathBuf, dir: &PathBuf) -> (Guard, PathBuf, u16, Logs) {
    let (crt, key) = generate_cert(dir, "127.0.0.1");
    let upstream_port = spawn_mock_upstream();
    let bind_port = free_port();

    let mut child = Command::new(bin)
        .env("TLS_CERT", &crt)
        .env("TLS_KEY", &key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{bind_port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", "qwen3.7-plus")
        .env("AI_API_KEY", "test-key-not-real")
        .env("AUTH_PEPPER", PEPPER)
        .env("SMS_KIND", "log")
        // 一个用例里要连续走注册和找回密码，默认 60 秒冷却跑不动
        .env("SMS_MIN_INTERVAL_SECS", "0")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动服务器子进程失败");

    let logs: Logs = Default::default();
    let out = child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>);
    let err = child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>);
    for pipe in [out, err].into_iter().flatten() {
        let logs = logs.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(pipe).lines().map_while(Result::ok) {
                logs.lock().unwrap_or_else(|e| e.into_inner()).push(line);
            }
        });
    }

    (Guard(child), crt, bind_port, logs)
}

fn extract_code(line: &str) -> Option<String> {
    let idx = line.find("验证码是 ")?;
    let rest = &line[idx + "验证码是 ".len()..];
    let code: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    (code.len() == 6).then_some(code)
}

async fn wait_for_code(logs: &Logs, after: usize) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let lines = logs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(code) = lines.iter().skip(after).rev().find_map(|l| extract_code(l)) {
            return code;
        }
        assert!(Instant::now() < deadline, "没等到验证码。日志：\n{}", lines.join("\n"));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn log_len(logs: &Logs) -> usize {
    logs.lock().unwrap_or_else(|e| e.into_inner()).len()
}

/// 用服务器自己的 --gen-license 出一张真码。
/// 校验位是拿 AUTH_PEPPER 算的 HMAC，客户端算不出来——这本身就是设计意图
/// （能算出来就等于把伪造能力跟着安装包分发出去了），所以测试也只能这么拿。
fn mint_license(bin: &PathBuf) -> String {
    let out = Command::new(bin)
        .arg("--gen-license")
        .arg("1")
        .env("AUTH_PEPPER", PEPPER)
        .output()
        .expect("跑 --gen-license 失败");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// 注意：remote.rs 的 HTTPS 客户端是进程级单例（OnceLock），一旦第一次用
/// 某个 SOUL_LANTERN_SERVER_BASE / 证书初始化，同进程里后续调用都复用同一
/// 个实例，环境变量之后再改也不会生效。所以这个文件只用一个测试函数覆盖
/// 全部场景，而不是拆成多个 #[tokio::test]——拆开会因为初始化时机不确定
/// 互相踩踏。
#[tokio::test]
async fn full_client_flow_against_real_server() {
    let Some(bin) = server_binary() else {
        eprintln!(
            "跳过 full_client_flow_against_real_server：找不到服务端二进制。\n\
             服务端在独立的私有仓库里，编译好之后用 SOUL_LANTERN_SERVER_BIN=<路径> 指过来即可。"
        );
        return;
    };

    let dir = std::env::temp_dir().join(format!("soul-client-it-{}-{}", std::process::id(), free_port()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (_guard, crt, bind_port, logs) = start_test_server(&bin, &dir);

    // Rust 2024 里改环境变量是 unsafe（进程全局可变状态）。这个文件只有一个
    // 测试函数、不会并行跑，这里是安全的。
    unsafe {
        std::env::set_var("SOUL_LANTERN_PINNED_CERT_FILE", &crt);
        std::env::set_var("SOUL_LANTERN_SERVER_BASE", format!("https://127.0.0.1:{bind_port}"));
        // session.rs 会往 dirs::config_dir() 写会话文件。不隔离的话测试会污染
        // 跑测试这台机器上真实用户的登录状态。
        std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
    }

    // 等服务器起来：轮询而不是固定 sleep（加了用户表加载/备份/写权限自检之后
    // 启动变慢，固定值会变成随机失败的 flaky 测试）
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if auth::auth_required().await.is_ok() && !logs.lock().unwrap().is_empty() {
            // auth_required 连不上时会静默返回 false，所以再用一次能区分成败的调用确认
            if billing::billing_topup_tiers().await.is_ok() {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "服务器 15 秒内没起来。日志：\n{}",
            logs.lock().unwrap().join("\n")
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // ---------- 未登录时该被挡住 ----------
    let state = auth::auth_state().await.expect("auth_state 本身不该报错");
    assert!(!state.logged_in, "全新环境应该是未登录");

    assert!(
        billing::billing_state().await.is_err(),
        "没登录查余额应该失败——改造前它是拿 device_id 就能查的"
    );
    assert!(
        billing::billing_recharge(1000).await.is_err(),
        "没登录不能充值（改造前 /v1/topup 是无鉴权免费发币口）"
    );

    let gated = ai::ai_generate(
        "system".to_string(), "做一把弓".to_string(), None, "java_1_21_5".to_string(), None,
    )
    .await
    .expect("ai_generate 本身返回 Ok，失败信息走 AiResponse.error");
    assert!(!gated.ok, "没登录时 AI 生成必须失败");
    assert!(gated.error.is_some());

    // ---------- 充值档位是公开接口，不需要登录 ----------
    let tiers = billing::billing_topup_tiers().await.expect("档位是公开接口");
    assert_eq!(tiers.len(), 8, "档位应该从服务器拿，不再是客户端各存一份静态表");
    assert_eq!((tiers[0].yuan, tiers[0].coins), (1.0, 1000));

    // ---------- 注册 ----------
    // 本地就能判断的错误不该跑一趟网络
    assert!(
        auth::auth_register_begin("小明".into(), "Passw0rd-2026".into(), "不一样的".into(), "13800138000".into())
            .await
            .is_err(),
        "两次密码不一致应该本地就拦下"
    );
    assert!(
        auth::auth_register_begin("小明".into(), "Passw0rd-2026".into(), "Passw0rd-2026".into(), "123".into())
            .await
            .is_err(),
        "手机号格式不对应该本地就拦下"
    );

    let before = log_len(&logs);
    let sent = auth::auth_register_begin(
        "小明".into(), "Passw0rd-2026".into(), "Passw0rd-2026".into(), "13800138000".into(),
    )
    .await
    .expect("注册第一步应该成功");
    assert_eq!(sent.phone_masked, "138****8000", "回显要打码，且字段名要对得上");
    assert!(sent.log_mode, "服务端是 SMS_KIND=log，客户端应该知道");

    let code = wait_for_code(&logs, before).await;
    let registered = auth::auth_register_verify("13800138000".into(), code)
        .await
        .expect("验证码正确应该完成注册");
    assert!(registered.logged_in);
    assert_eq!(registered.username, "小明");
    let welcome = registered.balance;
    assert!(welcome > 0, "注册应该发一次欢迎余额");

    // ---------- 登录态应该能从磁盘恢复 ----------
    let state = auth::auth_state().await.unwrap();
    assert!(state.logged_in, "token 已经落盘，重新问一次应该还是登录着");
    assert_eq!(state.username, "小明");

    // ---------- 现在这些接口该通了 ----------
    let bal = billing::billing_state().await.expect("登录后查余额应该成功");
    assert_eq!(bal.balance, welcome);

    assert!(billing::billing_recharge(999).await.is_err(), "非预设档位应该被服务器拒绝");
    let after_topup = billing::billing_recharge(1000).await.expect("预设档位充值应该成功");
    assert_eq!(after_topup.balance, welcome + 1000);

    // ---------- 激活码 ----------
    assert!(
        billing::billing_activate("NOPE".to_string()).await.is_err(),
        "格式明显不对的码本地就该拦下"
    );
    assert!(
        billing::billing_activate("SOUL-AAAA-AAAA-AAAA".to_string()).await.is_err(),
        "格式合法但校验位不对的伪造码必须被服务器拒绝——\
         这是改造前那个「36^12 个字符串每个都能兑 100 币」的洞的回归测试"
    );

    let license = mint_license(&bin);
    let activated = billing::billing_activate(license.clone()).await.expect("真码应该能兑换");
    assert_eq!(activated.balance, welcome + 1000 + 100, "激活应该叠加而不是覆盖");
    assert!(activated.activated);
    assert!(
        billing::billing_activate(license).await.is_err(),
        "同一个码不该能兑第二次"
    );

    // ---------- AI 生成：全链路 ----------
    let ai_resp = ai::ai_generate(
        "system prompt".to_string(), "做一把弓".to_string(), None, "java_1_21_5".to_string(), None,
    )
    .await
    .expect("ai_generate 本身不应该返回 Err");
    assert!(ai_resp.ok, "mock 上游应该让这次调用成功：{:?}", ai_resp.error);
    assert_eq!(
        ai_resp.commands,
        vec!["say hi".to_string()],
        "服务器应该已经把意图分派构建成命令字符串"
    );
    assert!(ai_resp.balance.is_some(), "成功调用后余额应该是具体数字而不是 None");
    assert!(ai_resp.balance.unwrap() < activated.balance, "成功调用应该扣了费");

    // ---------- 改密码：旧会话应该作废 ----------
    auth::auth_change_password("Passw0rd-2026".into(), "N3w-Passw0rd".into(), "N3w-Passw0rd".into())
        .await
        .expect("改密码应该成功");
    let after_change = auth::auth_state().await.unwrap();
    assert!(!after_change.logged_in, "改密之后本地会话应该已经作废");

    // ---------- 用新密码登录 ----------
    let relogin = auth::auth_login("小明".into(), "N3w-Passw0rd".into())
        .await
        .expect("新密码应该能登录");
    assert!(relogin.logged_in);

    assert!(
        auth::auth_login("小明".into(), "错的密码".into()).await.is_err(),
        "错密码应该登录失败"
    );

    // ---------- 找回密码 ----------
    let before = log_len(&logs);
    auth::auth_reset_begin("13800138000".into()).await.expect("找回密码第一步应该成功");
    let reset_code = wait_for_code(&logs, before).await;
    auth::auth_reset_confirm(
        "13800138000".into(), reset_code, "R3set-Passw0rd".into(), "R3set-Passw0rd".into(),
    )
    .await
    .expect("重置密码应该成功");

    let final_login = auth::auth_login("13800138000".into(), "R3set-Passw0rd".into())
        .await
        .expect("重置后的新密码应该能登录（这次用手机号登录）");
    assert!(final_login.logged_in);

    // ---------- 退出登录 ----------
    auth::auth_logout().await.unwrap();
    let out = auth::auth_state().await.unwrap();
    assert!(!out.logged_in, "退出登录后本地应该没有会话了");
}
