//! 官方目录数据 + 存在性校验。是 `dispatch.rs` 用来拦截 AI 编造 id 的地基，
//! 移植自客户端 `src/logic/dispatch.ts` 里 `indexed`/`catalogsFor`/`catalogMiss`
//! 那一段（数据来源见 `catalog_data.rs` 头部注释）。
//!
//! 数据本身在客户端和服务器各保留一份独立副本，不是共享单一数据源——见迁移
//! 计划里的说明，未来 `scripts/gen-catalog.mjs` 等脚本更新目录时要同时喂两边。

use std::collections::HashSet;
use std::sync::OnceLock;

pub use crate::give::catalog_data::*;

use crate::give::builder::GiveVersion;

/// 一份 (id, 中文名) 表 + 按 id 建好的存在性集合，避免每次校验都线性扫描。
pub struct Indexed {
    pub rows: &'static [(&'static str, &'static str)],
    ids: HashSet<&'static str>,
}

impl Indexed {
    fn new(rows: &'static [(&'static str, &'static str)]) -> Self {
        Self { rows, ids: rows.iter().map(|(id, _)| *id).collect() }
    }

    fn has(&self, id: &str) -> bool {
        self.ids.contains(id)
    }
}

/// 按 id 或中文名反查规范 id；匹配不上就当成"模组物品/自定义 id"原样放行
/// （加上 `minecraft:` 命名空间前缀，如果本来就没有的话）。
/// 对应客户端 `src/logic/builder.ts::mapCatalog`。
pub fn map_catalog(rows: &[(&str, &str)], text: &str) -> String {
    for (id, name) in rows {
        if text == *id || text == *name {
            return id.to_string();
        }
    }
    namespaced(text)
}

/// 对应客户端 `src/logic/builder.ts::namespaced`：没有命名空间前缀就补一个
/// `minecraft:`，已经带冒号（含其它命名空间，如 `mymod:foo`）就原样返回。
pub fn namespaced(text: &str) -> String {
    if text.contains(':') {
        text.to_string()
    } else {
        format!("minecraft:{text}")
    }
}

struct VersionCatalogs {
    items: Indexed,
    blocks: Indexed,
    entities: Indexed,
}

fn java_catalogs() -> &'static VersionCatalogs {
    static CELL: OnceLock<VersionCatalogs> = OnceLock::new();
    CELL.get_or_init(|| VersionCatalogs {
        items: Indexed::new(ITEMS),
        blocks: Indexed::new(BLOCKS),
        entities: Indexed::new(ENTITIES),
    })
}

fn bedrock_catalogs() -> &'static VersionCatalogs {
    static CELL: OnceLock<VersionCatalogs> = OnceLock::new();
    CELL.get_or_init(|| VersionCatalogs {
        items: Indexed::new(BEDROCK_ITEMS),
        blocks: Indexed::new(BEDROCK_BLOCKS),
        entities: Indexed::new(BEDROCK_ENTITIES),
    })
}

/// 附魔/药水效果/属性/粒子暂时两版共用 Java 表——原因见 dispatch.ts 同名注释：
/// 只生成了基岩物品/方块/实体三张表，且基岩 give 构建器本来就不输出附魔/效果。
fn enchant_cat() -> &'static Indexed {
    static CELL: OnceLock<Indexed> = OnceLock::new();
    CELL.get_or_init(|| Indexed::new(ENCHANTS))
}
fn effect_cat() -> &'static Indexed {
    static CELL: OnceLock<Indexed> = OnceLock::new();
    CELL.get_or_init(|| Indexed::new(EFFECTS))
}
fn attribute_cat() -> &'static Indexed {
    static CELL: OnceLock<Indexed> = OnceLock::new();
    CELL.get_or_init(|| Indexed::new(ATTRIBUTES))
}
fn particle_cat() -> &'static Indexed {
    static CELL: OnceLock<Indexed> = OnceLock::new();
    CELL.get_or_init(|| Indexed::new(PARTICLES))
}

fn catalogs_for(version: GiveVersion) -> &'static VersionCatalogs {
    if version == GiveVersion::Bedrock { bedrock_catalogs() } else { java_catalogs() }
}

/// 取粒子 id 里花括号之前的部分（`minecraft:dust{color:[...]}` -> `minecraft:dust`）。
/// 对应 dispatch.ts::particleIdOnly。
pub fn particle_id_only(raw: &str) -> &str {
    match raw.find('{') {
        Some(idx) => raw[..idx].trim(),
        None => raw,
    }
}

/// catalog 里没有的一律视为 AI 编造；命中就返回 Some(错误文案)。
/// 空字段留给各自的必填校验去报错，这里不重复报——对应 dispatch.ts::catalogMiss。
pub fn catalog_miss(kind: &str, raw: Option<&str>, cat: &Indexed) -> Option<String> {
    let raw = raw?;
    if raw.trim().is_empty() {
        return None;
    }
    if cat.has(&map_catalog(cat.rows, raw)) {
        return None;
    }
    Some(format!("{kind} \"{raw}\" 不在官方目录里，疑似 AI 编造，已拦截"))
}

pub struct GiveCatalogs;

impl GiveCatalogs {
    pub fn items(version: GiveVersion) -> &'static Indexed {
        &catalogs_for(version).items
    }
    pub fn blocks(version: GiveVersion) -> &'static Indexed {
        &catalogs_for(version).blocks
    }
    pub fn entities(version: GiveVersion) -> &'static Indexed {
        &catalogs_for(version).entities
    }
    pub fn enchants() -> &'static Indexed {
        enchant_cat()
    }
    pub fn effects() -> &'static Indexed {
        effect_cat()
    }
    pub fn attributes() -> &'static Indexed {
        attribute_cat()
    }
    pub fn particles() -> &'static Indexed {
        particle_cat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_match_ts_source() {
        // 与 gen-server-catalog.mjs 跑出的日志（同步跑 TS 版 catalog.ts 校验过）一致。
        assert_eq!(ITEMS.len(), 1537);
        assert_eq!(BLOCKS.len(), 1196);
        assert_eq!(ENTITIES.len(), 158);
        assert_eq!(PARTICLES.len(), 125);
        assert_eq!(BEDROCK_ITEMS.len(), 1487);
        assert_eq!(BEDROCK_BLOCKS.len(), 1141);
        assert_eq!(BEDROCK_ENTITIES.len(), 131);
        assert_eq!(ENCHANTS.len(), 43);
        assert_eq!(EFFECTS.len(), 39);
        assert_eq!(ATTRIBUTES.len(), 28);
    }

    #[test]
    fn java_web_is_cobweb_bedrock_web_is_web() {
        // 蜘蛛网：Java 叫 cobweb，基岩叫 web——这对是版本感知校验要防的经典案例。
        assert!(catalog_miss("物品", Some("minecraft:cobweb"), GiveCatalogs::items(GiveVersion::Java1_21_11Plus)).is_none());
        assert!(catalog_miss("物品", Some("minecraft:web"), GiveCatalogs::items(GiveVersion::Java1_21_11Plus)).is_some());
        assert!(catalog_miss("物品", Some("minecraft:web"), GiveCatalogs::items(GiveVersion::Bedrock)).is_none());
        assert!(catalog_miss("物品", Some("minecraft:cobweb"), GiveCatalogs::items(GiveVersion::Bedrock)).is_some());
    }

    #[test]
    fn empty_or_missing_field_is_not_a_catalog_miss() {
        assert!(catalog_miss("物品", Some(""), GiveCatalogs::items(GiveVersion::Java1_21_11Plus)).is_none());
        assert!(catalog_miss("物品", None, GiveCatalogs::items(GiveVersion::Java1_21_11Plus)).is_none());
    }

    #[test]
    fn made_up_id_is_rejected_with_expected_message() {
        let err = catalog_miss("物品", Some("minecraft:totally_fake_item"), GiveCatalogs::items(GiveVersion::Java1_21_11Plus));
        assert_eq!(err, Some("物品 \"minecraft:totally_fake_item\" 不在官方目录里，疑似 AI 编造，已拦截".to_string()));
    }

    #[test]
    fn particle_id_only_strips_brace_suffix() {
        assert_eq!(particle_id_only("minecraft:dust{color:[1.0,0.0,0.0],scale:1}"), "minecraft:dust");
        assert_eq!(particle_id_only("minecraft:flame"), "minecraft:flame");
    }

    #[test]
    fn map_catalog_matches_by_id_or_chinese_name() {
        let rows: &[(&str, &str)] = &[("minecraft:stone", "石头")];
        assert_eq!(map_catalog(rows, "minecraft:stone"), "minecraft:stone");
        assert_eq!(map_catalog(rows, "石头"), "minecraft:stone");
        assert_eq!(map_catalog(rows, "some_modded_thing"), "minecraft:some_modded_thing");
        assert_eq!(map_catalog(rows, "mymod:thing"), "mymod:thing");
    }
}
