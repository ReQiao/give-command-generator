//! 集成测试共用脚手架：起一个真实的服务器子进程 + 真实 TLS。
//!
//! 抽出来的原因是现在有两组集成测试（TLS 锁定、账号流程）都要做同样的事，
//! 而且"等服务器起来"这一步有个坑值得只写一遍：**不能用固定 sleep**。
//! 加了用户表加载、备份写入、写权限自检之后启动会变慢，原来那个
//! `sleep(800ms)` 有时候就不够了，会变成随机失败的 flaky 测试——
//! 而这类测试恰恰是客户端/服务端之间唯一的对接契约守门人，不能让它变得不可信。
//! 所以改成轮询 /v1/health 直到通，上限 15 秒。

#![allow(dead_code)] // 每个集成测试只用得到其中一部分

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub struct Guard(pub Child);

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
pub fn generate_cert(dir: &Path, name: &str, cn: &str) -> (PathBuf, PathBuf) {
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
/// 走完整条链路即可。
pub fn spawn_mock_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let body = r#"{"choices":[{"message":{"content":"{}"}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    port
}

/// 取一个当前空闲的端口。比"进程号取模"稳——后者在并行跑测试时会撞。
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soul-it-{tag}-{}-{}", std::process::id(), free_port()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub struct Server {
    pub port: u16,
    pub cert: PathBuf,
    pub dir: PathBuf,
    /// 子进程的 stdout/stderr。SMS_KIND=log 时验证码只出现在这里，
    /// 测试靠它拿到码——顺便也就真的把"短信挂了之后从 journalctl 捞码"
    /// 这条救火通道测了一遍，而不只是假设它能用。
    logs: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    _guard: Guard,
}

pub struct ServerOptions {
    pub sms_kind: &'static str,
    pub admin_token: Option<&'static str>,
    pub auth_pepper: &'static str,
    pub sms_min_interval_secs: &'static str,
    /// 自定义 mock 上游端口。不给就起一个只回固定内容的默认上游。
    pub upstream_port: Option<u16>,
    pub ai_model: &'static str,
}

impl Default for ServerOptions {
    fn default() -> Self {
        ServerOptions {
            sms_kind: "log",
            admin_token: None,
            auth_pepper: "integration-test-pepper-do-not-use-in-prod",
            sms_min_interval_secs: "0",
            upstream_port: None,
            ai_model: "qwen3.7-plus",
        }
    }
}

/// 起服务器并**等它真的能响应**再返回。
pub async fn start_server(tag: &str, opts: ServerOptions) -> Server {
    let dir = temp_dir(tag);
    let (crt, key) = generate_cert(&dir, "server", "127.0.0.1");
    let upstream_port = opts.upstream_port.unwrap_or_else(spawn_mock_upstream);
    let port = free_port();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soul-lantern-server"));
    cmd.env("TLS_CERT", &crt)
        .env("TLS_KEY", &key)
        .env("LEDGER_PATH", dir.join("ledger.json"))
        .env("BIND_ADDR", format!("127.0.0.1:{port}"))
        .env("AI_ENDPOINT", format!("http://127.0.0.1:{upstream_port}/mock"))
        .env("AI_MODEL", opts.ai_model)
        .env("AI_API_KEY", "test-key-not-real")
        .env("AUTH_PEPPER", opts.auth_pepper)
        .env("SMS_KIND", opts.sms_kind)
        // 同一手机号的发码冷却默认 60 秒，一个用例里连续走注册和找回密码就跑不动了。
        // 这里设成 0 是刻意的：这条限流的**语义**（register/resend/reset 共用一个
        // 计数器）由 sms_rate_limit_blocks_rapid_resend 那个用例单独覆盖，
        // 它自己会把这个值设回非零。
        .env("SMS_MIN_INTERVAL_SECS", opts.sms_min_interval_secs)
        // Log 后端的验证码是用 error! 打的，但其它诊断信息要 info 才看得到
        .env("RUST_LOG", "info");
    if let Some(t) = opts.admin_token {
        cmd.env("ADMIN_TOKEN", t);
    }

    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("启动服务器子进程失败");

    let logs: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    for pipe in [
        child.stdout.take().map(PipeSource::Out),
        child.stderr.take().map(PipeSource::Err),
    ]
    .into_iter()
    .flatten()
    {
        let logs = logs.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader: Box<dyn Read + Send> = match pipe {
                PipeSource::Out(o) => Box::new(o),
                PipeSource::Err(e) => Box::new(e),
            };
            for line in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
                logs.lock().unwrap_or_else(|e| e.into_inner()).push(line);
            }
        });
    }

    let guard = Guard(child);
    let client = pinned_client(&crt);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if client.get(format!("https://127.0.0.1:{port}/v1/health")).send().await.is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            let captured = logs.lock().unwrap_or_else(|e| e.into_inner()).join("\n");
            panic!("服务器 15 秒内没起来。子进程输出：\n{captured}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Server { port, cert: crt, dir, logs, _guard: guard }
}

enum PipeSource {
    Out(std::process::ChildStdout),
    Err(std::process::ChildStderr),
}

pub fn pinned_client(cert_path: &Path) -> reqwest::Client {
    let cert_pem = std::fs::read(cert_path).unwrap();
    let cert = reqwest::Certificate::from_pem(&cert_pem).unwrap();
    reqwest::Client::builder()
        .tls_built_in_root_certs(false) // 关键：只信任下面这一张，不信任公共 CA
        .add_root_certificate(cert)
        .build()
        .unwrap()
}

impl Server {
    pub fn client(&self) -> reqwest::Client {
        pinned_client(&self.cert)
    }

    pub fn url(&self, path: &str) -> String {
        format!("https://127.0.0.1:{}{path}", self.port)
    }

    pub fn captured_logs(&self) -> Vec<String> {
        self.logs.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 从日志里捞出最近一条验证码。轮询是必要的：发信是在 handler 里同步做的，
    /// 但日志写到管道、测试这边读到管道之间隔着线程调度。
    pub async fn wait_for_code(&self, after_line_count: usize) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let lines = self.captured_logs();
            if let Some(code) = lines
                .iter()
                .skip(after_line_count)
                .rev()
                .find_map(|l| extract_code(l))
            {
                return code;
            }
            if Instant::now() >= deadline {
                panic!("5 秒内没在日志里等到验证码。日志：\n{}", lines.join("\n"));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub fn log_line_count(&self) -> usize {
        self.captured_logs().len()
    }
}

impl Server {
    /// 走完整的注册流程拿一个可用的 Bearer token。
    /// 很多测试的真正目标在注册之后，这一段每次手写太啰嗦。
    pub async fn register_and_login(&self, username: &str, phone: &str, password: &str) -> String {
        let c = self.client();
        let before = self.log_line_count();
        let begin = c
            .post(self.url("/v1/auth/register/begin"))
            .json(&serde_json::json!({
                "username": username, "password": password, "phone": phone
            }))
            .send()
            .await
            .expect("register/begin 请求失败");
        assert_eq!(begin.status(), 200, "register/begin 应该成功：{:?}", begin.text().await);

        let code = self.wait_for_code(before).await;
        let verify = c
            .post(self.url("/v1/auth/register/verify"))
            .json(&serde_json::json!({ "phone": phone, "code": code }))
            .send()
            .await
            .expect("register/verify 请求失败");
        assert_eq!(verify.status(), 200, "register/verify 应该成功");
        let body: serde_json::Value = verify.json().await.unwrap();
        body["token"].as_str().expect("响应里应该有 token").to_string()
    }
}

/// 从 `……验证码是 123456（300秒内有效）……` 里抠出那串数字。
/// 不引 regex：找锚点关键字再吃连续数字就够了。
fn extract_code(line: &str) -> Option<String> {
    let idx = line.find("验证码是 ")?;
    let rest = &line[idx + "验证码是 ".len()..];
    let code: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    (code.len() == 6).then_some(code)
}
