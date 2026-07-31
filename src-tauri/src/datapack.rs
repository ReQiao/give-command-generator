//! 一键部署：扫描 .minecraft/saves，把生成的命令打包成 datapack 写进存档。
//!
//! 为什么走 datapack 而不是替玩家把命令打进聊天栏：后者需要模拟输入或注入客户端，
//! 那是外挂的做法。datapack 是原版官方的内容分发方式——我们只是往存档目录写文件，
//! 玩家自己在游戏里执行 `/reload` 加载，全程没有任何非官方手段。
//!
//! 一次性命令 vs 循环命令：
//!   - commands（一次性）写进 soul:run，玩家自己执行一次 `/function soul:run` 触发。
//!   - loop_commands（需要每 tick 侦测的，如箭矢/掉落物落地检测）写进 soul:tick，
//!     并通过 `data/minecraft/tags/function/tick.json` 挂到原版的 tick 函数标签上——
//!     `/reload` 之后自动每 tick 执行，不需要玩家再手动放一个循环命令方块。
//!     这是这次改造的关键点：凡是「持续侦测」的组合技，一键部署即可生效。

use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// datapack 的命名空间与函数名，最终对应游戏内的 `/function soul:run`。
const NAMESPACE: &str = "soul";
const FUNCTION: &str = "run";
const TICK_FUNCTION: &str = "tick";
const PACK_DIR: &str = "soul_lantern_commands";

#[derive(Clone, Serialize)]
pub struct SaveInfo {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    /// datapack 写入的目录。
    pub pack_path: String,
    /// 一次性命令条数（写进 soul:run，需要玩家手动触发一次）。
    pub command_count: usize,
    /// 循环命令条数（写进 soul:tick 并挂 tick.json，`/reload` 后自动生效）。
    pub loop_command_count: usize,
    /// 玩家需要在游戏内执行的命令：/reload 必做；run_command 只有 command_count>0 时才需要，
    /// 循环部分 /reload 后自动生效，不需要玩家再做任何事。
    pub reload_command: String,
    pub run_command: Option<String>,
}

/// 定位 .minecraft 目录（各平台默认位置）。
fn minecraft_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\.minecraft
        dirs::config_dir()
            .map(|d| d.join(".minecraft"))
            .ok_or_else(|| "无法定位 %APPDATA% 目录".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|d| d.join("Library/Application Support/minecraft"))
            .ok_or_else(|| "无法定位用户主目录".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir()
            .map(|d| d.join(".minecraft"))
            .ok_or_else(|| "无法定位用户主目录".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("不支持的操作系统".to_string())
    }
}

/// 扫描 .minecraft/saves，列出所有存档。
///
/// 只认带 level.dat 的目录——saves 下常混有截图、备份之类的杂物，
/// 全列出来会让用户在下拉框里选到一个根本不是世界的目录。
#[tauri::command]
pub fn datapack_list_saves(saves_dir: Option<String>) -> Result<Vec<SaveInfo>, String> {
    let dir = match saves_dir {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => minecraft_dir()?.join("saves"),
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&dir).map_err(|e| format!("读取 saves 目录失败：{e}"))?;
    let mut saves: Vec<SaveInfo> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().join("level.dat").is_file())
        .filter_map(|e| {
            Some(SaveInfo {
                name: e.file_name().to_str()?.to_string(),
                path: e.path().to_str()?.to_string(),
            })
        })
        .collect();

    saves.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(saves)
}

/// 目标版本 → datapack 的 pack_format。
///
/// 取值来自各版本 server.jar 内 version.json 的 pack_version.data_major
/// （26.2 已实测为 107，见 scripts/mc-verifier/cache/26.2）。
/// 中间几个版本没有逐一实测，所以 pack.mcmeta 里额外写了 supported_formats 兜底，
/// 即便这里的数字偏差，datapack 仍能被游戏接受。
fn pack_format_for_version(version: &str) -> i32 {
    match version {
        "java_1_20_5" => 41,
        "java_1_21" | "java_1_21_1" => 48,
        "java_1_21_2" | "java_1_21_3" => 57,
        "java_1_21_4" => 61,
        "java_1_21_5" => 71,
        "java_1_21_6" => 80,
        "java_1_21_9" => 88,
        "java_26_1" => 99,
        _ => 107, // 26.2+（实测值）
    }
}

/// 命令存进 .mcfunction 时不能带前导斜杠。空行与注释行原样保留。
fn clean_commands(commands: &[String]) -> Vec<String> {
    commands
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.strip_prefix('/').unwrap_or(c).to_string())
        .collect()
}

/// 把命令写成 datapack 放进指定存档。
/// `commands` 是一次性命令（玩家手动触发一次）；`loop_commands` 是需要每 tick
/// 侦测的命令（自动挂 tick.json，`/reload` 后无需再做任何事）——两者可以只有一个非空。
#[tauri::command]
pub fn datapack_deploy(
    save_path: String,
    commands: Vec<String>,
    loop_commands: Vec<String>,
    version: String,
) -> Result<DeployResult, String> {
    let save = PathBuf::from(&save_path);
    if !save.join("level.dat").is_file() {
        return Err(format!("{save_path} 不像是一个存档目录（没有 level.dat）。"));
    }

    let body = clean_commands(&commands);
    let tick_body = clean_commands(&loop_commands);
    if body.is_empty() && tick_body.is_empty() {
        return Err("没有可部署的命令。".to_string());
    }

    let pack_dir = save.join("datapacks").join(PACK_DIR);
    // 1.21 起函数目录由 functions 改名为 function（单数）；两处都写，跨版本都能加载。
    let function_dirs = [
        pack_dir.join("data").join(NAMESPACE).join("function"),
        pack_dir.join("data").join(NAMESPACE).join("functions"),
    ];
    for dir in &function_dirs {
        fs::create_dir_all(dir).map_err(|e| format!("创建 datapack 目录失败：{e}"))?;
    }

    let format = pack_format_for_version(&version);
    let meta = serde_json::json!({
        "pack": {
            "pack_format": format,
            "description": "Soul Lantern 生成的指令",
            // 官方字段（1.20.2+）：声明兼容区间，避免 pack_format 与玩家版本
            // 差一点就整个加载不了。
            "supported_formats": { "min_inclusive": 41, "max_inclusive": 9999 }
        }
    });
    fs::write(pack_dir.join("pack.mcmeta"), serde_json::to_vec_pretty(&meta).unwrap_or_default())
        .map_err(|e| format!("写入 pack.mcmeta 失败：{e}"))?;

    let mut run_command = None;
    if !body.is_empty() {
        let content = format!("{}\n", body.join("\n"));
        for dir in &function_dirs {
            fs::write(dir.join(format!("{FUNCTION}.mcfunction")), &content)
                .map_err(|e| format!("写入 {FUNCTION}.mcfunction 失败：{e}"))?;
        }
        run_command = Some(format!("/function {NAMESPACE}:{FUNCTION}"));
    }

    if !tick_body.is_empty() {
        let content = format!("{}\n", tick_body.join("\n"));
        for dir in &function_dirs {
            fs::write(dir.join(format!("{TICK_FUNCTION}.mcfunction")), &content)
                .map_err(|e| format!("写入 {TICK_FUNCTION}.mcfunction 失败：{e}"))?;
        }
        // 1.21 起函数标签目录同样由 functions 改名为 function；两处都写。
        let tag_dirs = [
            pack_dir.join("data/minecraft/tags/function"),
            pack_dir.join("data/minecraft/tags/functions"),
        ];
        let tag = serde_json::json!({ "values": [format!("{NAMESPACE}:{TICK_FUNCTION}")] });
        let tag_bytes = serde_json::to_vec_pretty(&tag).unwrap_or_default();
        for dir in &tag_dirs {
            fs::create_dir_all(dir).map_err(|e| format!("创建 tick.json 目录失败：{e}"))?;
            fs::write(dir.join("tick.json"), &tag_bytes).map_err(|e| format!("写入 tick.json 失败：{e}"))?;
        }
    }

    Ok(DeployResult {
        pack_path: pack_dir.to_string_lossy().to_string(),
        command_count: body.len(),
        loop_command_count: tick_body.len(),
        reload_command: "/reload".to_string(),
        run_command,
    })
}

/// 猜测 .minecraft/saves 的默认位置，用于给「浏览选择存档」的文件夹选择框一个起始路径。
/// 猜不到（非官方启动器目录结构不同）或目录不存在时返回 None，前端会退回系统默认位置。
#[tauri::command]
pub fn datapack_default_saves_dir() -> Option<String> {
    let dir = minecraft_dir().ok()?.join("saves");
    dir.is_dir().then(|| dir.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 测试用的薄封装：省去每处都写 to_string 转换。一次性命令，无循环命令。
    fn deploy_for_test(save: &Path, commands: &[&str], version: &str) -> Result<DeployResult, String> {
        deploy_for_test_full(save, commands, &[], version)
    }

    fn deploy_for_test_full(
        save: &Path,
        commands: &[&str],
        loop_commands: &[&str],
        version: &str,
    ) -> Result<DeployResult, String> {
        datapack_deploy(
            save.to_string_lossy().to_string(),
            commands.iter().map(|s| s.to_string()).collect(),
            loop_commands.iter().map(|s| s.to_string()).collect(),
            version.to_string(),
        )
    }

    fn temp_save(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soul-datapack-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("level.dat"), b"fake").unwrap();
        dir
    }

    #[test]
    fn writes_pack_and_function() {
        let save = temp_save("basic");
        let res = deploy_for_test(
            &save,
            &["/give @s minecraft:mace 1", "say hi"],
            "java_26_2_plus",
        )
        .unwrap();

        assert_eq!(res.command_count, 2);
        assert_eq!(res.loop_command_count, 0);
        assert_eq!(res.run_command, Some("/function soul:run".to_string()));

        let pack = save.join("datapacks").join(PACK_DIR);
        let meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(pack.join("pack.mcmeta")).unwrap()).unwrap();
        assert_eq!(meta["pack"]["pack_format"], 107);
        assert_eq!(meta["pack"]["supported_formats"]["min_inclusive"], 41);

        // 前导斜杠必须去掉，否则游戏加载 function 时会报错
        let body = fs::read_to_string(pack.join("data/soul/function/run.mcfunction")).unwrap();
        assert_eq!(body, "give @s minecraft:mace 1\nsay hi\n");

        // 旧版单复数目录都要写
        assert!(pack.join("data/soul/functions/run.mcfunction").is_file());

        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn picks_pack_format_by_version() {
        assert_eq!(pack_format_for_version("java_1_20_5"), 41);
        assert_eq!(pack_format_for_version("java_1_21_4"), 61);
        assert_eq!(pack_format_for_version("java_26_2_plus"), 107);
    }

    #[test]
    fn rejects_non_save_dir() {
        let dir = std::env::temp_dir().join("soul-datapack-test-notasave");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let err = deploy_for_test(&dir, &["say hi"], "java_26_2_plus").unwrap_err();
        assert!(err.contains("level.dat"), "应提示这不是存档目录，实际：{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_empty_commands() {
        let save = temp_save("empty");
        assert!(deploy_for_test(&save, &["", "   "], "java_26_2_plus").is_err());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn writes_tick_function_and_tag_for_loop_commands() {
        let save = temp_save("loop");
        let res = deploy_for_test_full(
            &save,
            &[],
            &[
                "execute at @e[type=minecraft:arrow,nbt={inGround:1b}] run summon minecraft:tnt ~ ~ ~ {fuse:0s}",
                "execute as @e[type=minecraft:arrow,nbt={inGround:1b}] run kill @s",
            ],
            "java_26_2_plus",
        )
        .unwrap();

        assert_eq!(res.command_count, 0);
        assert_eq!(res.loop_command_count, 2);
        // 没有一次性命令时不需要玩家手动触发任何东西
        assert_eq!(res.run_command, None);

        let pack = save.join("datapacks").join(PACK_DIR);
        let tick_body = fs::read_to_string(pack.join("data/soul/function/tick.mcfunction")).unwrap();
        assert!(tick_body.contains("summon minecraft:tnt"));
        assert!(tick_body.contains("run kill @s"));

        // tick.json 挂到原版的 tick 函数标签上，/reload 后自动每 tick 执行
        let tag: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(pack.join("data/minecraft/tags/function/tick.json")).unwrap())
                .unwrap();
        assert_eq!(tag["values"][0], "soul:tick");
        // 旧版单复数目录都要写
        assert!(pack.join("data/minecraft/tags/functions/tick.json").is_file());
        assert!(pack.join("data/soul/functions/tick.mcfunction").is_file());

        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn supports_both_once_and_loop_commands_together() {
        let save = temp_save("mixed");
        let res = deploy_for_test_full(&save, &["say hello"], &["say tick"], "java_26_2_plus").unwrap();
        assert_eq!(res.command_count, 1);
        assert_eq!(res.loop_command_count, 1);
        assert_eq!(res.run_command, Some("/function soul:run".to_string()));

        let pack = save.join("datapacks").join(PACK_DIR);
        assert!(pack.join("data/soul/function/run.mcfunction").is_file());
        assert!(pack.join("data/soul/function/tick.mcfunction").is_file());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn rejects_when_both_command_lists_are_empty() {
        let save = temp_save("empty-both");
        assert!(deploy_for_test_full(&save, &[], &[], "java_26_2_plus").is_err());
        fs::remove_dir_all(&save).unwrap();
    }

    #[test]
    fn lists_only_real_saves() {
        let root = std::env::temp_dir().join("soul-datapack-test-saves");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("WorldB")).unwrap();
        fs::write(root.join("WorldB/level.dat"), b"x").unwrap();
        fs::create_dir_all(root.join("WorldA")).unwrap();
        fs::write(root.join("WorldA/level.dat"), b"x").unwrap();
        fs::create_dir_all(root.join("screenshots")).unwrap(); // 杂物目录，无 level.dat

        let saves = datapack_list_saves(Some(root.to_string_lossy().to_string())).unwrap();
        let names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["WorldA", "WorldB"], "只列真存档且按名排序");

        fs::remove_dir_all(&root).unwrap();
    }
}
