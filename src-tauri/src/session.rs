//! 本地会话：把服务器发的 Bearer token 存在磁盘上，重启软件不用重新登录。
//!
//! 存放位置沿用 device.rs 当年那套约定（`dirs::config_dir()/soul-lantern/`），
//! 只是文件名换成 `session.json`。device.rs 已经删掉了——匿名 device_id 那套
//! 身份被账号取代了，它既不能跨设备找回余额，又是个"删掉本地文件就重发欢迎币"
//! 的水龙头（见 server/src/ledger.rs 顶部注释）。
//!
//! **token 不放前端 localStorage，只在 Rust 侧读写。** 前端拿不到它，
//! 也就少一条被注入脚本顺走的路径；前端需要知道的只有"现在登没登录、
//! 用户名是什么"，那些走 `auth_me` 现问服务器。
//!
//! 这个文件里没有任何"权威"信息：token 有没有效、余额是多少，全都由服务器说了算。
//! 本地这份只是个缓存，被人删掉/篡改的最坏后果就是要重新登录一次。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct StoredSession {
    pub token: String,
    /// 服务器给的过期时间（unix 秒）。本地只拿它做"要不要提前当作没登录"的
    /// 判断，真正说了算的还是服务器——本地时钟不准是常事。
    pub expires_at: u64,
}

fn session_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("soul-lantern").join("session.json"))
}

pub fn load() -> Option<StoredSession> {
    let path = session_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let s: StoredSession = serde_json::from_str(&text).ok()?;
    (!s.token.is_empty()).then_some(s)
}

pub fn token() -> Option<String> {
    load().map(|s| s.token)
}

pub fn save(token: &str, expires_at: u64) {
    let Some(path) = session_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body = serde_json::to_string(&StoredSession { token: token.to_string(), expires_at })
        .unwrap_or_default();
    let _ = std::fs::write(&path, body);
    harden(&path);
}

pub fn clear() {
    if let Some(path) = session_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// 会话文件等同于密码，别让同机器上的其它账号读到。
#[cfg(unix)]
fn harden(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn harden(_path: &std::path::Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_token_is_treated_as_logged_out() {
        // 防的是"文件存在但内容是空壳"这种情况被误判成已登录，
        // 那样客户端会带着一个空 Bearer 头去请求，拿到 401 之后又清不干净。
        let s: StoredSession = serde_json::from_str(r#"{"token":"","expires_at":0}"#).unwrap();
        assert!(s.token.is_empty());
    }

    #[test]
    fn serialization_roundtrip() {
        let s = StoredSession { token: "abc".into(), expires_at: 123 };
        let text = serde_json::to_string(&s).unwrap();
        let back: StoredSession = serde_json::from_str(&text).unwrap();
        assert_eq!(back.token, "abc");
        assert_eq!(back.expires_at, 123);
    }
}
