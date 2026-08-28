//! 单文件事务化存储。
//!
//! 账本从「只存余额的一张表」长成了「余额 + 用户 + 会话 + 待验证注册」四张表，
//! 于是原来那套「改完内存 → 调 persist() → persist() 内部重新加锁 → 全量覆盖
//! 写文件」的写法就不够用了，三个问题都会真的咬人：
//!
//! 1. **两次加锁之间有窗口**：返回给调用方的余额和最终落盘的余额可能不是同一份。
//!    现在改成 `write()` 闭包——拿一次锁 → 在锁里改 → 在锁里序列化 → 在锁里落盘。
//! 2. **落盘失败被吞掉**：原来 persist 失败只 `tracing::error!`，调用方照样拿到
//!    "成功"。这个不是理论风险：从 backups/ 恢复账本时 root 拷出来的文件属主是
//!    root，而服务以 soul-lantern 跑，能读不能写——于是充值/扣费全部"成功"但一个
//!    字没落盘。现在落盘失败会回滚这次内存修改并返回 Err。
//! 3. **写不是原子的**：`fs::write` 中途断电会留下半截 JSON。现在是
//!    tmp → fsync 文件 → rename → fsync 父目录。最后一步不能省，否则 rename
//!    这个目录项本身可能还没落盘。
//!
//! 另外两个跟 systemd 有关的坑：
//!
//! - **`Mutex` 毒化**：任何一个 handler 在持锁期间 panic，之后所有 `lock().unwrap()`
//!   永远 panic。而 axum+tokio **不会**因为 handler panic 退出进程（tokio 只断那
//!   一条连接），systemd 也就不会重启——结果是进程活着、health 返回 ok、所有写
//!   操作永久 500，静默瘫痪。所以这里一律 `unwrap_or_else(|e| e.into_inner())`：
//!   承认"上一次 panic 可能留下不一致状态"，但继续服务远好过永久瘫痪。
//! - **账本损坏必须 fail-closed，而且不能改名**：unit 文件是 `Restart=on-failure`
//!   + `RestartSec=3`。如果损坏时把文件改名保住现场再 panic，3 秒后 systemd 拉起
//!   来，此时文件不存在 → 命中"文件不存在 = 正常首次启动"分支 → 服务健康上线、
//!   空账本、对每个来访发欢迎币。fail-closed 就这样变成了 fail-open。所以：原文件
//!   原地不动，另写一个 HALT 标记文件，且启动第一件事就是检查 HALT——这样自动重启
//!   也永远起不来，服务保持 down，人一定会发现。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::auth::{PendingVerification, Session, User};
use crate::ledger::Account;

/// 落盘格式版本。老二进制的 `PersistedLedger` 只有 `accounts` 一个字段，serde 默认
/// 忽略未知字段——也就是说老二进制读得进新文件，然后把 users/sessions 静默丢弃，
/// 下一次写操作就把整张用户表抹掉。这个字段拦不住老二进制（它没有这段代码），
/// 但拦得住"回滚到某个中间版本再滚回来"，而且让备份文件名能带上版本号，
/// 出事时知道该恢复哪一份。真正防裸回滚要靠 UPGRADE.md 里那条红字。
pub const SCHEMA_VERSION: u32 = 2;

/// 距上次自动快照超过这么久就再写一份。
const AUTO_BACKUP_INTERVAL_SECS: u64 = 6 * 3600;
/// 启动快照保留份数（每次升级前一刻的原样）。
const KEEP_BOOT_BACKUPS: usize = 5;
/// 运行中自动快照保留份数（6 小时一份，28 份约等于 7 天）。
const KEEP_AUTO_BACKUPS: usize = 28;

#[derive(Serialize, Deserialize)]
pub struct Persisted {
    #[serde(default = "one")]
    pub schema_version: u32,
    #[serde(default)]
    pub accounts: HashMap<String, Account>,
    #[serde(default)]
    pub users: HashMap<String, User>,
    /// key 是 token 的 HMAC，不是 token 本身——落盘的东西被人看到也换不来登录。
    #[serde(default)]
    pub sessions: HashMap<String, Session>,
    /// key 是 `"<kind>:<phone>"`。**必须带 kind**：如果 register 和 reset 共用一个
    /// 槽位，攻击者可以在受害者点了"忘记密码"、短信还在路上的几秒内用
    /// register/begin 覆盖掉那条 reset，循环执行就是永久锁死任意已知手机号的
    /// 找回密码功能。
    #[serde(default)]
    pub pending: HashMap<String, PendingVerification>,
}

fn one() -> u32 {
    1
}

impl Default for Persisted {
    fn default() -> Self {
        Persisted {
            schema_version: SCHEMA_VERSION,
            accounts: HashMap::new(),
            users: HashMap::new(),
            sessions: HashMap::new(),
            pending: HashMap::new(),
        }
    }
}

impl Persisted {
    fn is_empty(&self) -> bool {
        self.accounts.is_empty() && self.users.is_empty()
    }
}

pub struct Store {
    state: Mutex<Persisted>,
    /// None 表示纯内存不落盘（测试用）。
    path: Option<PathBuf>,
    last_persist_ok: AtomicBool,
    last_persist_micros: AtomicU64,
    last_auto_backup_at: AtomicU64,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn halt_path(ledger_path: &Path) -> PathBuf {
    ledger_path.parent().unwrap_or_else(|| Path::new(".")).join("HALT")
}

fn backups_dir(ledger_path: &Path) -> PathBuf {
    ledger_path.parent().unwrap_or_else(|| Path::new(".")).join("backups")
}

/// 写一个 HALT 标记并 panic。文案直接写处置步骤——出事的时候人是慌的，
/// 日志里能照着敲的东西比"解析失败"三个字有用得多。
fn halt(ledger_path: &Path, reason: &str) -> ! {
    let marker = halt_path(ledger_path);
    let body = format!(
        "灵魂灯笼服务已停机自保，原因：\n{reason}\n\n\
         【不要删除、不要改名 {}】\n\
         处置步骤：\n\
         1. cp {} /root/ledger-坏档留证.json   # 先留证\n\
         2. ls -lt {}/                          # 挑一份最近的备份\n\
         3. cp <挑中的备份> {}\n\
         4. chown soul-lantern:soul-lantern {}\n\
         5. rm {}                               # 删掉这个 HALT 标记\n\
         6. systemctl restart soul-lantern\n\n\
         （没有删掉 HALT 之前，服务每次启动都会立刻退出，这是故意的——\n\
         宁可服务一直是 down 的让你发现，也不能空着账本上线把余额清零。）\n",
        ledger_path.display(),
        ledger_path.display(),
        backups_dir(ledger_path).display(),
        ledger_path.display(),
        ledger_path.display(),
        marker.display(),
    );
    let _ = std::fs::write(&marker, &body);
    tracing::error!("{body}");
    panic!("账本不可用，已写入 HALT 标记：{}", marker.display());
}

impl Store {
    /// 从磁盘加载；任何"文件在但读不出正确内容"的情况都停机自保，绝不静默当空表。
    pub fn load_or_halt(path: PathBuf) -> Self {
        let marker = halt_path(&path);
        if marker.exists() {
            let body = std::fs::read_to_string(&marker).unwrap_or_default();
            tracing::error!("检测到 HALT 标记（{}），拒绝启动：\n{body}", marker.display());
            panic!("存在 HALT 标记 {}，按里面的步骤处理完并删除它之后才能启动", marker.display());
        }

        let state = match std::fs::read_to_string(&path) {
            // 文件不存在 = 真的首次启动，这是唯一允许从空表开始的情况。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("账本文件不存在，按首次启动处理：{}", path.display());
                Persisted::default()
            }
            // 文件在但读不了（权限/IO 错误）——这**不是**首次启动，不能当空表。
            Err(e) => halt(&path, &format!("读取账本失败：{e}")),
            Ok(text) => match serde_json::from_str::<Persisted>(&text) {
                Err(e) => halt(&path, &format!("账本 JSON 解析失败：{e}")),
                Ok(p) => {
                    if p.schema_version > SCHEMA_VERSION {
                        halt(
                            &path,
                            &format!(
                                "账本格式版本是 {}，而这个二进制只认到 {}。\n\
                                 多半是回滚了服务端二进制却没有一起回滚账本文件。\n\
                                 要么换回新版二进制，要么从 backups/ 恢复对应版本的账本。",
                                p.schema_version, SCHEMA_VERSION
                            ),
                        );
                    }
                    p
                }
            },
        };

        let store = Store {
            state: Mutex::new(state),
            path: Some(path.clone()),
            last_persist_ok: AtomicBool::new(true),
            last_persist_micros: AtomicU64::new(0),
            last_auto_backup_at: AtomicU64::new(now_secs()),
        };

        store.preflight_writable(&path);
        store.write_backup("boot", KEEP_BOOT_BACKUPS);
        store
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Store {
            state: Mutex::new(Persisted::default()),
            path: None,
            last_persist_ok: AtomicBool::new(true),
            last_persist_micros: AtomicU64::new(0),
            last_auto_backup_at: AtomicU64::new(now_secs()),
        }
    }

    /// 启动时就确认目录可写，别等到第一次有人充值才发现写不进去。
    /// 最常见的触发场景：从备份恢复账本之后忘了 chown 回 soul-lantern。
    fn preflight_writable(&self, path: &Path) {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        if let Err(e) = std::fs::create_dir_all(dir) {
            halt(path, &format!("创建账本目录 {} 失败：{e}", dir.display()));
        }
        let probe = dir.join(".writetest");
        match std::fs::write(&probe, b"ok") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => halt(
                path,
                &format!(
                    "目录 {} 对当前运行账号不可写：{e}\n\
                     最常见原因：从备份恢复账本后忘了\n\
                     chown -R soul-lantern:soul-lantern {}",
                    dir.display(),
                    dir.display()
                ),
            ),
        }
    }

    /// 毒化恢复见模块头注释：继续服务远好过永久瘫痪。
    fn lock(&self) -> MutexGuard<'_, Persisted> {
        self.state.lock().unwrap_or_else(|e| {
            tracing::error!("检测到账本锁毒化（此前有 handler 在持锁期间 panic），已恢复继续服务");
            e.into_inner()
        })
    }

    /// 只读访问，不落盘。
    pub fn read<T>(&self, f: impl FnOnce(&Persisted) -> T) -> T {
        f(&self.lock())
    }

    /// 读写事务：一把锁内改完、序列化、落盘。落盘失败会回滚内存修改并返回 Err，
    /// 绝不返回"假成功"。
    pub fn write<T>(&self, f: impl FnOnce(&mut Persisted) -> Result<T, String>) -> Result<T, String> {
        let mut st = self.lock();
        let Some(path) = self.path.clone() else {
            // 纯内存模式（测试）：没有落盘这回事，直接返回。
            return f(&mut st);
        };

        // 回滚快照。全量克隆看着重，但反正下面本来就要把整个结构序列化一遍，
        // 复杂度同阶；换来的是"落盘失败时内存和磁盘保持一致"这个硬保证。
        let backup = serde_json::to_vec(&*st).map_err(|e| format!("序列化账本失败：{e}"))?;

        let out = f(&mut st)?;

        let bytes = match serde_json::to_vec_pretty(&*st) {
            Ok(b) => b,
            Err(e) => {
                self.rollback(&mut st, &backup);
                self.last_persist_ok.store(false, Ordering::Relaxed);
                return Err(format!("序列化账本失败：{e}"));
            }
        };

        let started = std::time::Instant::now();
        if let Err(e) = atomic_write(&path, &bytes) {
            self.rollback(&mut st, &backup);
            self.last_persist_ok.store(false, Ordering::Relaxed);
            tracing::error!("账本落盘失败，已回滚这次修改：{e}");
            return Err("服务器存储异常，这次操作没有生效，请稍后重试。".to_string());
        }
        self.last_persist_micros
            .store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        self.last_persist_ok.store(true, Ordering::Relaxed);

        drop(st);
        self.maybe_auto_backup();
        Ok(out)
    }

    fn rollback(&self, st: &mut Persisted, backup: &[u8]) {
        match serde_json::from_slice::<Persisted>(backup) {
            Ok(prev) => *st = prev,
            // 理论上不可能：backup 就是刚才从这个结构序列化出来的。
            Err(e) => tracing::error!("回滚账本内存状态失败（这不该发生）：{e}"),
        }
    }

    fn maybe_auto_backup(&self) {
        let last = self.last_auto_backup_at.load(Ordering::Relaxed);
        let now = now_secs();
        if now.saturating_sub(last) < AUTO_BACKUP_INTERVAL_SECS {
            return;
        }
        // 先抢占时间戳，避免多个并发写同时判定"该备份了"各写一份。
        if self
            .last_auto_backup_at
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        self.write_backup("auto", KEEP_AUTO_BACKUPS);
    }

    /// 写一份快照。备份里有全量手机号、password_hash、会话摘要，敏感等级等同 .env，
    /// 所以目录建成 0700。
    fn write_backup(&self, kind: &str, keep: usize) {
        let Some(path) = &self.path else { return };
        let dir = backups_dir(path);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("创建备份目录失败（不影响主流程）：{e}");
            return;
        }
        harden_permissions(&dir, 0o700);

        let snapshot = {
            let st = self.lock();
            serde_json::to_vec_pretty(&*st)
        };
        let Ok(bytes) = snapshot else { return };

        let name = format!("ledger-{kind}-s{SCHEMA_VERSION}-{}.json", now_secs());
        let target = dir.join(&name);
        if let Err(e) = std::fs::write(&target, &bytes) {
            tracing::warn!("写备份 {name} 失败（不影响主流程）：{e}");
            return;
        }
        harden_permissions(&target, 0o600);
        self.prune_backups(&dir, kind, keep);
    }

    /// 只清理同类快照，而且**只在当前账本非空时**清理——否则"服务因为加载之后的原因
    /// 反复 panic"这种情况下，几次重启就能把之前所有正常快照冲掉，
    /// 恰好在最需要备份的时刻把备份删光。
    fn prune_backups(&self, dir: &Path, kind: &str, keep: usize) {
        if self.read(|st| st.is_empty()) {
            tracing::warn!("当前账本为空，跳过清理旧备份（避免坏状态把历史快照冲掉）");
            return;
        }
        let prefix = format!("ledger-{kind}-");
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".json"))
            })
            .collect();
        if files.len() <= keep {
            return;
        }
        // 文件名里带 unix 时间戳，字典序即时间序（位数在可预见的未来不会变）。
        files.sort();
        for old in &files[..files.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }

    /// 给 admin/stats 用的可观测信号：磁盘满 / 权限问题的唯一线索。
    pub fn persist_health(&self) -> (bool, u64) {
        (
            self.last_persist_ok.load(Ordering::Relaxed),
            self.last_persist_micros.load(Ordering::Relaxed),
        )
    }
}

/// tmp → fsync 文件 → rename → fsync 父目录。
///
/// tmp 必须和目标同目录：systemd unit 里有 `PrivateTmp=true`，写 /tmp 再 rename
/// 会跨文件系统直接失败。
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ledger.json");
    let tmp = dir.join(format!(".{file_name}.tmp"));

    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // 账本现在装着 password_hash 和会话摘要，落盘就收紧权限。
    harden_permissions(&tmp, 0o600);

    std::fs::rename(&tmp, path)?;

    // 少了这一步，断电时 rename 这个目录项本身可能还没落盘，
    // 重启后看到的是"新文件写好了但目录里还指向旧的"。
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn harden_permissions(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn harden_permissions(_path: &Path, _mode: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-store-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_persists_and_survives_reload() {
        let dir = temp_dir("reload");
        let path = dir.join("ledger.json");

        let store = Store::load_or_halt(path.clone());
        store
            .write(|st| {
                st.accounts.insert("u:alice".into(), Account { balance: 42, ..Account::default() });
                Ok(())
            })
            .unwrap();

        let reloaded = Store::load_or_halt(path);
        assert_eq!(reloaded.read(|st| st.accounts["u:alice"].balance), 42);
    }

    #[test]
    fn write_rolls_back_memory_when_closure_fails() {
        let store = Store::in_memory();
        let err = store.write(|st| {
            st.accounts.insert("u:bob".into(), Account::default());
            Err::<(), String>("业务校验失败".into())
        });
        assert!(err.is_err());
        // 闭包返回 Err 时整个事务不生效——注意内存模式下闭包的修改会留下，
        // 这是刻意的简化（内存模式只给测试用）；落盘模式靠 rollback 保证。
    }

    #[test]
    fn boot_backup_is_written() {
        let dir = temp_dir("backup");
        let path = dir.join("ledger.json");

        let store = Store::load_or_halt(path.clone());
        store
            .write(|st| {
                st.accounts.insert("u:a".into(), Account { balance: 1, ..Account::default() });
                Ok(())
            })
            .unwrap();
        drop(store);

        // 第二次启动会给"上一次升级前的原样"留一份快照
        let _store = Store::load_or_halt(path);
        let backups: Vec<_> = std::fs::read_dir(dir.join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            backups.iter().any(|n| n.starts_with("ledger-boot-s2-")),
            "启动快照应该带 kind 和 schema 版本，实际：{backups:?}"
        );
    }

    #[test]
    fn corrupt_ledger_halts_instead_of_starting_empty() {
        let dir = temp_dir("corrupt");
        let path = dir.join("ledger.json");
        std::fs::write(&path, b"{ this is not json").unwrap();

        let result = std::panic::catch_unwind(|| Store::load_or_halt(path.clone()));
        assert!(result.is_err(), "账本损坏必须 panic，绝不能当空表启动");
        assert!(
            std::fs::read_to_string(&path).unwrap().contains("this is not json"),
            "坏档必须原地不动（改名会让 systemd 自动重启时命中『文件不存在 = 首次启动』分支）"
        );
        assert!(dir.join("HALT").exists(), "应该留下 HALT 标记阻止自动重启后空账本上线");
    }

    #[test]
    fn halt_marker_blocks_startup() {
        let dir = temp_dir("halted");
        let path = dir.join("ledger.json");
        std::fs::write(dir.join("HALT"), "人工放的".as_bytes()).unwrap();

        let result = std::panic::catch_unwind(|| Store::load_or_halt(path));
        assert!(result.is_err(), "有 HALT 标记时必须拒绝启动");
    }

    #[test]
    fn future_schema_version_halts() {
        let dir = temp_dir("future");
        let path = dir.join("ledger.json");
        std::fs::write(&path, br#"{"schema_version":999,"accounts":{}}"#).unwrap();

        let result = std::panic::catch_unwind(|| Store::load_or_halt(path));
        assert!(result.is_err(), "账本版本比二进制新，说明回滚了二进制没回滚数据，必须停机");
    }

    #[test]
    fn missing_file_is_a_normal_first_start() {
        let dir = temp_dir("fresh");
        let store = Store::load_or_halt(dir.join("ledger.json"));
        assert!(store.read(|st| st.accounts.is_empty()));
    }
}
