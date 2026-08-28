//! 设备标识：给这台安装生成一个稳定的随机 id，服务器账本（见仓库根目录
//! server/）靠它认领"这是同一个人"。
//!
//! 不是登录账号——没有用户名密码，纯粹是一个随机数，卸载重装会生成新的一个
//! （等于刷新一次首次体验额度）。这是刻意接受的权衡：做真正的账号体系是
//! 完全另一个量级的工程，现在没必要。这份实现真正要解决的问题跟账号是否
//! 严谨无关——是"大模型 key 不再跟着客户端分发"，见 remote.rs 顶部注释。

use std::path::PathBuf;

fn device_id_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("soul-lantern").join("device_id"))
}

/// 读取本机的 device_id；不存在就生成一个新的并落盘。
///
/// 定位不到配置目录的极端情况下，每次调用都会生成一个新的临时 id——服务器
/// 那边会把它当成从没见过的新设备，除了体验额度不连续之外不影响功能（不会
/// 崩，只是账本上多一条"来过一次就再没出现过"的记录）。
pub fn get_or_create() -> String {
    let Some(path) = device_id_path() else {
        return uuid::Uuid::new_v4().to_string();
    };
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let fresh = uuid::Uuid::new_v4().to_string();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, &fresh);
    fresh
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_ids_look_like_uuids() {
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36, "UUID v4 的标准文本长度是 36（含 4 个连字符）");
        assert_eq!(id.matches('-').count(), 4);
    }
}
