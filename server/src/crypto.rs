//! 加密相关的小工具，全部基于 `ring`。
//!
//! 为什么是 ring 而不是 argon2 / bcrypt / sha2 这些更常见的选择：ring 本来就已经在
//! 依赖树里（rustls 的加密后端就是它），用它意味着这次账号体系**零新增加密依赖**。
//! Argon2 理论上比 PBKDF2 更抗 GPU，但它的内存参数（m=19456 KiB ≈ 每次哈希 19 MiB）
//! 在一台小内存 ECS 上是个实打实的 OOM 风险——注册接口被脚本打一分钟，
//! 50 个并发就是 950 MiB。PBKDF2 只吃 CPU，配合下面的有界信号量更好控。
//!
//! 真正决定"泄库之后密码好不好破"的，其实是弱密码策略而不是迭代次数：
//! 600k 次 PBKDF2-HMAC-SHA256 在一张 4090 上大约 5~10 kH/s，
//! 而一个弱口令字典只有几万条。所以 auth.rs 那边的弱密码黑名单比这里的参数重要。

use ring::rand::SecureRandom;

/// PBKDF2 迭代次数。OWASP 对 PBKDF2-HMAC-SHA256 的建议量级。
/// 存进每条用户记录里而不是写死读取——以后调大了，老用户下次登录成功时可以顺手升级。
pub const PBKDF2_ITERATIONS: u32 = 600_000;
pub const HASH_LEN: usize = 32;
pub const SALT_LEN: usize = 16;

/// 会话 token 的原始字节数。32 字节 = 256 bit，够了。
pub const TOKEN_BYTES: usize = 32;

fn rng() -> &'static ring::rand::SystemRandom {
    use std::sync::OnceLock;
    static RNG: OnceLock<ring::rand::SystemRandom> = OnceLock::new();
    RNG.get_or_init(ring::rand::SystemRandom::new)
}

pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    // SystemRandom 读的是操作系统熵源，失败基本等同于系统本身出了大问题；
    // 这种情况下继续用一个可预测的值比直接崩更危险。
    rng().fill(&mut buf).expect("系统随机数源不可用");
    buf
}

/// 生成一个 URL 安全的随机 token（会话 token 用）。
pub fn random_token() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes(TOKEN_BYTES))
}

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    ring::hmac::sign(&k, msg).as_ref().to_vec()
}

/// 阿里云 RPC 签名用的是 HMAC-SHA1。ring 把它标成 "FOR_LEGACY_USE_ONLY"，
/// 这里正是那个 legacy use：算法是阿里云接口定死的，不是我们能选的。
pub fn hmac_sha1(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    ring::hmac::sign(&k, msg).as_ref().to_vec()
}

/// 定长比较。比对 token 摘要、验证码、激活码校验位时都要用它，
/// 不要用 `==`（短路比较会泄露"前几个字节对了"这个信息）。
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    ring::constant_time::verify_slices_are_equal(a, b).is_ok()
}

pub fn pbkdf2_hash(password: &str, salt: &[u8], iterations: u32) -> Vec<u8> {
    let mut out = vec![0u8; HASH_LEN];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(iterations).expect("迭代次数不能是 0"),
        salt,
        password.as_bytes(),
        &mut out,
    );
    out
}

/// `ring::pbkdf2::verify` 本身就是常量时间的，外面不需要再套一层手工比较。
pub fn pbkdf2_verify(password: &str, salt: &[u8], iterations: u32, expected: &[u8]) -> bool {
    let Some(iters) = std::num::NonZeroU32::new(iterations) else { return false };
    ring::pbkdf2::verify(ring::pbkdf2::PBKDF2_HMAC_SHA256, iters, salt, password.as_bytes(), expected)
        .is_ok()
}

// ---------------------------------------------------------------- 激活码

/// 激活码字符集：去掉了 I/O/0/1 这四个手抄时最容易看错的字符。
/// 刚好 32 个 = 5 bit/字符，映射的时候不用做取模偏置处理。
const LICENSE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn map_to_alphabet(bytes: &[u8], len: usize) -> String {
    bytes
        .iter()
        .take(len)
        .map(|b| LICENSE_ALPHABET[(*b as usize) % LICENSE_ALPHABET.len()] as char)
        .collect()
}

/// 从序列号算出 4 位校验段。
///
/// 这一段是修「激活码可无限伪造」那个洞的核心：改造前 `is_valid_license` 只校验
/// 格式，于是 `SOUL-AAAA-AAAA-AAAB`、`...AAAC` 这类约 36^12 个字符串每一个都是
/// 一张能兑 100 币的一次性券，写个 for 循环就能刷。加上校验段之后，
/// 伪造需要先拿到只存在于服务器环境变量里的 pepper。
fn check_segment(pepper: &[u8], serial: &str) -> String {
    let mac = hmac_sha256(pepper, format!("license:{serial}").as_bytes());
    map_to_alphabet(&mac, 4)
}

/// 生成一张激活码。给 `--gen-license` 子命令用（在服务器上离线批量出码）。
pub fn generate_license(pepper: &[u8]) -> String {
    let raw = random_bytes(8);
    let a = map_to_alphabet(&raw[0..4], 4);
    let b = map_to_alphabet(&raw[4..8], 4);
    let serial = format!("{a}{b}");
    format!("SOUL-{a}-{b}-{}", check_segment(pepper, &serial))
}

/// 规范化：去掉空白和连字符之外的杂质、统一大写。用户从聊天记录里复制常常带空格。
pub fn normalize_license(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// 校验激活码：格式 + 校验段。校验段用常量时间比较。
pub fn verify_license(pepper: &[u8], key: &str) -> bool {
    let flat = normalize_license(key);
    // SOUL + 4 + 4 + 4 = 16 个字符
    if flat.len() != 16 || !flat.starts_with("SOUL") {
        return false;
    }
    let body = &flat[4..];
    if !body.bytes().all(|b| LICENSE_ALPHABET.contains(&b)) {
        return false;
    }
    let serial = &body[0..8];
    let given = &body[8..12];
    constant_time_eq(given.as_bytes(), check_segment(pepper, serial).as_bytes())
}

/// 展示用的规范形态 SOUL-XXXX-YYYY-ZZZZ（落账本时统一存这个形态，方便人工核对）。
pub fn canonical_license(key: &str) -> String {
    let flat = normalize_license(key);
    if flat.len() != 16 {
        return flat;
    }
    format!("SOUL-{}-{}-{}", &flat[4..8], &flat[8..12], &flat[12..16])
}

// ---------------------------------------------------------------- 手机号 / 打码

/// 只接受中国大陆手机号：1 开头、第二位 3~9、共 11 位数字。
/// 阿里云短信认证服务本身也只支持中国大陆号码，这里提前挡住能省一次无谓的计费调用。
pub fn normalize_phone(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    // 容忍用户带上 +86 / 86 前缀
    let digits = digits.strip_prefix("86").unwrap_or(&digits).to_string();
    let ok = digits.len() == 11
        && digits.starts_with('1')
        && digits.as_bytes()[1].is_ascii_digit()
        && (b'3'..=b'9').contains(&digits.as_bytes()[1]);
    ok.then_some(digits)
}

/// 回显给客户端时打码，别把完整手机号又发回去。
pub fn mask_phone(phone: &str) -> String {
    if phone.len() != 11 {
        return "***".to_string();
    }
    format!("{}****{}", &phone[0..3], &phone[7..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEPPER: &[u8] = "test-pepper-不要用在生产".as_bytes();

    #[test]
    fn pbkdf2_roundtrip() {
        let salt = random_bytes(SALT_LEN);
        // 测试里用低迭代，不然每个用例都要跑几百毫秒
        let hash = pbkdf2_hash("hunter2", &salt, 1000);
        assert!(pbkdf2_verify("hunter2", &salt, 1000, &hash));
        assert!(!pbkdf2_verify("hunter3", &salt, 1000, &hash));
    }

    #[test]
    fn generated_license_verifies() {
        for _ in 0..50 {
            let key = generate_license(PEPPER);
            assert!(verify_license(PEPPER, &key), "自己生成的码必须能通过校验：{key}");
        }
    }

    #[test]
    fn forged_license_is_rejected() {
        // 这是改造前那个洞的回归测试：格式完全合法但不是我们签出来的码
        assert!(!verify_license(PEPPER, "SOUL-AAAA-AAAA-AAAB"));
        assert!(!verify_license(PEPPER, "SOUL-AAAA-AAAA-AAAC"));
        assert!(!verify_license(PEPPER, "SOUL-2345-6789-ABCD"));
    }

    #[test]
    fn license_from_a_different_pepper_is_rejected() {
        let key = generate_license(b"another-pepper");
        assert!(!verify_license(PEPPER, &key), "换了 pepper 就该认不出来，否则 pepper 没起作用");
    }

    #[test]
    fn license_normalization_tolerates_copy_paste_noise() {
        let key = generate_license(PEPPER);
        let noisy = format!(" {} ", key.to_lowercase().replace('-', " — "));
        assert!(verify_license(PEPPER, &noisy), "从聊天记录复制带上空格/破折号也该认");
    }

    #[test]
    fn phone_normalization() {
        assert_eq!(normalize_phone("13800138000").as_deref(), Some("13800138000"));
        assert_eq!(normalize_phone("+86 138 0013 8000").as_deref(), Some("13800138000"));
        assert_eq!(normalize_phone("138-0013-8000").as_deref(), Some("13800138000"));
        assert!(normalize_phone("12800138000").is_none(), "第二位是 2 不是合法号段");
        assert!(normalize_phone("1380013800").is_none(), "10 位不合法");
        assert!(normalize_phone("").is_none());
    }

    #[test]
    fn phone_masking() {
        assert_eq!(mask_phone("13800138000"), "138****8000");
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
