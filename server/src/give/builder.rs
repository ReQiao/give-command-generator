//! give 指令核心构建器。移植自客户端 `src/logic/builder.ts`。
//!
//! **当前状态：仅 `GiveVersion` 类型骨架**，`buildGiveCommand` 本体和依赖的
//! 富文本/SNBT 工具函数尚未移植（下一阶段任务）。这个类型骨架先落地是因为
//! `catalog.rs` 的版本感知目录选择（Java vs 基岩）需要依赖它。

use serde::{Deserialize, Serialize};

/// 对应客户端 `src/logic/builder.ts::GiveVersion`——13 个变体必须逐一对应，
/// 顺序/拼写任何差异都会导致服务器和客户端对"这是哪个版本"的理解对不上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiveVersion {
    #[serde(rename = "java_1_20_5")]
    Java1_20_5,
    #[serde(rename = "java_1_21")]
    Java1_21,
    #[serde(rename = "java_1_21_1")]
    Java1_21_1,
    #[serde(rename = "java_1_21_2")]
    Java1_21_2,
    #[serde(rename = "java_1_21_3")]
    Java1_21_3,
    #[serde(rename = "java_1_21_4")]
    Java1_21_4,
    #[serde(rename = "java_1_21_5")]
    Java1_21_5,
    #[serde(rename = "java_1_21_6")]
    Java1_21_6,
    #[serde(rename = "java_1_21_9")]
    Java1_21_9,
    #[serde(rename = "java_1_21_11_plus")]
    Java1_21_11Plus,
    #[serde(rename = "java_26_1")]
    Java26_1,
    #[serde(rename = "java_26_2_plus")]
    Java26_2Plus,
    #[serde(rename = "bedrock")]
    Bedrock,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_tags_match_ts_string_literals() {
        // 与客户端 GiveVersion 联合类型的字符串字面量逐一核对。
        let cases: &[(GiveVersion, &str)] = &[
            (GiveVersion::Java1_20_5, "\"java_1_20_5\""),
            (GiveVersion::Java1_21, "\"java_1_21\""),
            (GiveVersion::Java1_21_1, "\"java_1_21_1\""),
            (GiveVersion::Java1_21_2, "\"java_1_21_2\""),
            (GiveVersion::Java1_21_3, "\"java_1_21_3\""),
            (GiveVersion::Java1_21_4, "\"java_1_21_4\""),
            (GiveVersion::Java1_21_5, "\"java_1_21_5\""),
            (GiveVersion::Java1_21_6, "\"java_1_21_6\""),
            (GiveVersion::Java1_21_9, "\"java_1_21_9\""),
            (GiveVersion::Java1_21_11Plus, "\"java_1_21_11_plus\""),
            (GiveVersion::Java26_1, "\"java_26_1\""),
            (GiveVersion::Java26_2Plus, "\"java_26_2_plus\""),
            (GiveVersion::Bedrock, "\"bedrock\""),
        ];
        for (version, expected) in cases {
            assert_eq!(serde_json::to_string(version).unwrap(), *expected);
            let parsed: GiveVersion = serde_json::from_str(expected).unwrap();
            assert_eq!(parsed, *version);
        }
    }
}
