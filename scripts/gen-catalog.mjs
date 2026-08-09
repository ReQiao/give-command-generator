#!/usr/bin/env node
/**
 * 从 Mojang 官方数据源生成 src/data/items.generated.ts
 *
 * 数据来源全部为官方渠道，不涉及反编译：
 *   1. piston-meta.mojang.com  版本清单 -> 指定版本的 server.jar
 *   2. server.jar 自带的数据生成器 (net.minecraft.data.Main --reports)
 *      产出 registries.json，即该版本全部物品 / 方块的注册表
 *   3. 官方资源索引 -> assets 里的 zh_cn.json，官方简体中文译名
 *
 * 用法:
 *   node scripts/gen-catalog.mjs              # 生成最新正式版
 *   node scripts/gen-catalog.mjs 26.2         # 指定版本
 *   node scripts/gen-catalog.mjs --snapshot   # 允许使用最新快照
 *
 * 需要 Java 25+（新版 server.jar 的运行时要求），仅用于跑官方数据生成器。
 */

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ensureServerJar, loadManifest } from "./mc-verifier/mojang.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
// 复用语法验证器的 server.jar 缓存（带 sha1 校验，且已在 .gitignore 中）
const CACHE = join(ROOT, "scripts", "mc-verifier", "cache");
const OUT = join(ROOT, "src", "data", "items.generated.ts");

const args = process.argv.slice(2);
const useSnapshot = args.includes("--snapshot");
const wantVersion = args.find((a) => !a.startsWith("--"));

async function getJson(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} <- ${url}`);
  return res.json();
}

async function download(url, dest) {
  if (existsSync(dest)) return dest;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} <- ${url}`);
  writeFileSync(dest, Buffer.from(await res.arrayBuffer()));
  return dest;
}

function findJava() {
  const candidates = [
    process.env.JAVA_HOME && join(process.env.JAVA_HOME, "bin", "java"),
    "/usr/lib/jvm/java-25-openjdk-amd64/bin/java",
    "/usr/lib/jvm/java-26-openjdk-amd64/bin/java",
    "java",
  ].filter(Boolean);

  for (const java of candidates) {
    try {
      // 用 --version（Java 9+）而不是 -version：前者写 stdout，后者写 stderr
      const text = execFileSync(java, ["--version"], {
        stdio: ["ignore", "pipe", "pipe"],
        encoding: "utf8",
        env: { ...process.env, JAVA_TOOL_OPTIONS: "" },
      });
      const major = Number(/(?:openjdk|java) (\d+)/i.exec(text)?.[1] ?? 0);
      if (major >= 25) return java;
    } catch {
      // 试下一个
    }
  }
  throw new Error(
    "未找到 Java 25+。新版 server.jar 的数据生成器需要 Java 25 或更高版本。\n" +
      "  Debian/Ubuntu: apt-get install -y openjdk-25-jdk-headless",
  );
}

/* ---------- 分类 ---------- */

// 命中即归类，顺序敏感：越具体的规则越靠前
const BLOCK_RULES = [
  ["红石", /redstone|piston|(^|_)rail$|_rail$|hopper|dispenser|dropper|observer|comparator|repeater|lever|button$|pressure_plate|tripwire|target$|daylight_detector|note_block|lightning_rod|_bulb$|calibrated|sculk_sensor|sculk_shrieker/],
  ["染色方块", /^(white|orange|magenta|light_blue|yellow|lime|pink|gray|light_gray|cyan|purple|blue|brown|green|red|black)_(wool|carpet|terracotta|concrete|concrete_powder|stained_glass|stained_glass_pane|bed|banner|wall_banner|shulker_box|candle|glazed_terracotta)$|^(wool|carpet|terracotta|shulker_box|candle)$/],
  ["自然方块", /_ore$|^(coal|iron|copper|gold|redstone|emerald|lapis|diamond|quartz|netherite)_block$|_log$|_wood$|_leaves$|_sapling$|^dirt|grass|podzol|mycelium|_sand$|^sand$|gravel|clay$|coral|_flower$|tulip|orchid|allium|bluet|daisy|poppy|dandelion|rose|lilac|peony|sunflower|mushroom|^ice$|_ice$|snow|^water$|^lava$|_plant$|vine|kelp|seagrass|bamboo|cactus|sponge|obsidian|amethyst|budding|moss|sculk$|sculk_vein|sculk_catalyst|dripleaf|azalea|mangrove_roots|muddy|_mud$|^mud$|pointed_dripstone|glow_lichen|spore_blossom|hanging_roots|nether_wart_block|shroomlight|_fungus$|_roots$|weeping_vines|twisting_vines|chorus_(plant|flower)|turtle_egg|frogspawn|sniffer_egg|dragon_egg|cobweb|_crop$|wheat|carrots|potatoes|beetroots|melon|pumpkin|sugar_cane|cocoa|sweet_berry|cave_vines|pale_moss|pale_hanging_moss|creaking_heart|firefly_bush|leaf_litter|wildflowers|bush$|cactus_flower|dried_ghast/],
  ["功能方块", /crafting_table|crafter|furnace|smoker|chest$|_chest$|barrel|_door$|_trapdoor$|fence_gate|torch|lantern|_sign$|_hanging_sign$|bell$|anvil|enchanting_table|brewing_stand|beacon|conduit|bookshelf|campfire|cauldron|composter|grindstone|loom$|smithing_table|stonecutter|scaffolding|ladder|jukebox|lectern|respawn_anchor|beehive|bee_nest|glass_pane$|iron_bars|_chain$|^chain$|end_rod|flower_pot|_head$|_skull$|spawner|portal|end_gateway|structure_|jigsaw|command_block|barrier|light$|air$|_pot$|decorated_pot|vault$|trial_spawner|copper_golem_statue|lightning|shelf$/],
  ["建筑方块", /.*/],
];

const FOOD = new Set([
  "apple", "golden_apple", "enchanted_golden_apple", "bread", "cookie", "cake", "pumpkin_pie",
  "melon_slice", "sweet_berries", "glow_berries", "chorus_fruit", "dried_kelp", "honey_bottle",
  "milk_bucket", "mushroom_stew", "rabbit_stew", "beetroot_soup", "suspicious_stew",
  "beef", "cooked_beef", "porkchop", "cooked_porkchop", "chicken", "cooked_chicken",
  "mutton", "cooked_mutton", "rabbit", "cooked_rabbit", "cod", "cooked_cod",
  "salmon", "cooked_salmon", "tropical_fish", "pufferfish", "rotten_flesh", "spider_eye",
  "carrot", "golden_carrot", "potato", "baked_potato", "poisonous_potato", "beetroot",
  "ominous_bottle",
]);

const ITEM_RULES = [
  ["刷怪蛋", /_spawn_egg$/],
  ["唱片", /^music_disc_|^disc_fragment/],
  ["药水", /^potion$|^splash_potion$|^lingering_potion$|^tipped_arrow$|^glass_bottle$|^experience_bottle$/],
  ["食物", (path) => FOOD.has(path)],
  ["战斗", /_sword$|^mace$|_spear$|^bow$|^crossbow$|^arrow$|^spectral_arrow$|^shield$|^trident$|_helmet$|_chestplate$|_leggings$|_boots$|^turtle_helmet$|_horse_armor$|^wolf_armor$|^elytra$|^totem_of_undying$|^firework_rocket$|^fire_charge$|^wind_charge$/],
  ["工具", /_pickaxe$|_axe$|_shovel$|_hoe$|^shears$|^fishing_rod$|^flint_and_steel$|_bucket$|^bucket$|^brush$|^spyglass$|^compass$|^recovery_compass$|^clock$|^lead$|^name_tag$|^saddle$|_on_a_stick$|^goat_horn$|^map$|^filled_map$|^ender_eye$|^ender_pearl$|^chorus_fruit$|^书$|^writable_book$|^written_book$|^knowledge_book$|^bundle$|_bundle$|^harness$|_harness$/],
  ["原材料", /.*/],
];

function classify(path, isBlock) {
  const rules = isBlock ? BLOCK_RULES : ITEM_RULES;
  for (const [name, rule] of rules) {
    if (typeof rule === "function" ? rule(path) : rule.test(path)) return name;
  }
  return "其他";
}

/* ---------- 生成 ---------- */

function zhName(zh, path, isBlock) {
  // 方块物品用 block.* 键，纯物品用 item.* 键，两边都试
  const keys = isBlock
    ? [`block.minecraft.${path}`, `item.minecraft.${path}`]
    : [`item.minecraft.${path}`, `block.minecraft.${path}`];
  for (const k of keys) if (zh[k]) return zh[k];
  return path.replace(/_/g, " ");
}

/**
 * 唱片 / 锻造模板 / 旗帜图案这几类，官方 item.* 键是共用的通用名
 *（21 张唱片都叫「音乐唱片」），真正区分它们的名字在另外的键里。
 * 找得到就用更精确的名字，找不到就返回 null 走通用的英文 ID 消歧。
 */
function preciseName(zh, path) {
  let m;

  if ((m = /^music_disc_(.+)$/.exec(path))) {
    const song = zh[`jukebox_song.minecraft.${m[1]}`];
    if (song) return `音乐唱片·${song.split(" - ").pop().trim()}`;
  }

  if ((m = /^(.+)_armor_trim_smithing_template$/.exec(path))) {
    const trim = zh[`trim_pattern.minecraft.${m[1]}`];
    if (trim) return `锻造模板·${trim.replace(/盔甲纹饰$/, "")}`;
  }

  if ((m = /^(.+)_smithing_template$/.exec(path))) {
    const up = zh[`upgrade.minecraft.${m[1]}`];
    if (up) return `锻造模板·${up}`;
  }

  if ((m = /^(.+)_banner_pattern$/.exec(path))) {
    // 旗帜盾徽名带颜色前缀，取黑色那条再把「黑色」去掉
    const banner = zh[`block.minecraft.banner.${m[1]}.black`];
    if (banner) return `旗帜图案·${banner.replace(/^黑色/, "")}`;
  }

  return null;
}

// summon.entityType 的分类，纯粹为了和其它目录格式一致，不影响存在性校验。
const ENTITY_RULES = [
  ["载具", /_boat$|_chest_boat$|_raft$|_chest_raft$|^minecart|_minecart$/],
  ["投掷物", /^arrow$|^spectral_arrow$|^trident$|^snowball$|^egg$|^ender_pearl$|^experience_bottle$|^potion$|^lingering_potion$|^(small_|dragon_)?fireball$|^wither_skull$|^shulker_bullet$|^llama_spit$|^(breeze_)?wind_charge$|^fishing_bobber$|^firework_rocket$/],
  ["非生物实体", /^item$|^item_frame$|^glow_item_frame$|^painting$|^armor_stand$|^end_crystal$|^falling_block$|^tnt$|^leash_knot$|^marker$|^interaction$|^(text|item|block)_display$|^area_effect_cloud$|^evoker_fangs$|^lightning_bolt$|^experience_orb$/],
  ["玩家", /^player$/],
  ["生物", /.*/],
];

function classifyEntity(path) {
  for (const [name, rule] of ENTITY_RULES) if (rule.test(path)) return name;
  return "其他";
}

/** entity_type 注册表用官方 entity.minecraft.<path> 语言键取中文名，比物品/方块简单得多。 */
function buildEntities(ids, zh) {
  const seen = new Map();
  const rows = [];
  for (const id of ids) {
    const path = id.slice("minecraft:".length);
    let name = zh[`entity.minecraft.${path}`] || path.replace(/_/g, " ");
    if (seen.has(name)) name = `${name}(${path})`;
    seen.set(name, id);
    rows.push([id, name, path.replace(/_/g, " "), classifyEntity(path)]);
  }
  return rows;
}

/**
 * particle_type 注册表。
 *
 * 和物品/方块/实体不同，官方语言文件里**没有** particle.minecraft.<path> 这一族键
 * （只有 particle.invalidOptions / particle.notFound 两条错误提示），所以拿不到
 * 官方中文译名。这里一律退回英文名，不自己编译名——粒子 id 本来就是给指令用的，
 * 不是给玩家在背包里看的，编一套没有官方依据的中文反而容易出错。
 */
function buildParticles(ids) {
  return ids.map((id) => {
    const path = id.slice("minecraft:".length);
    const en = path.replace(/_/g, " ");
    return [id, en, en, "粒子"];
  });
}

function build(ids, zh, blockSet) {
  const seen = new Map();
  const rows = [];

  for (const id of ids) {
    const path = id.slice("minecraft:".length);
    const isBlock = blockSet.has(id);
    let name = preciseName(zh, path) || zhName(zh, path, isBlock);

    // 中文重名会让 mapCatalog / form.item 无法区分，附加英文 ID 消歧
    if (seen.has(name)) {
      name = `${name}(${path})`;
    }
    seen.set(name, id);

    rows.push([id, name, path.replace(/_/g, " "), classify(path, isBlock)]);
  }
  return rows;
}

function serialize(rows) {
  return rows.map((r) => `  ${JSON.stringify(r)},`).join("\n");
}

async function main() {
  mkdirSync(CACHE, { recursive: true });

  const manifest = await loadManifest();
  const version = wantVersion || (useSnapshot ? manifest.latest.snapshot : manifest.latest.release);
  const entry = manifest.versions.find((v) => v.id === version);
  if (!entry) throw new Error(`版本清单里没有 ${version}`);
  console.log(`[1/5] 版本 ${version} (${entry.type}, ${entry.releaseTime})`);

  const meta = await getJson(entry.url);

  console.log("[2/5] 准备 server.jar …");
  const jar = await ensureServerJar(version, CACHE, (m) => console.log(m));

  console.log("[3/5] 运行官方数据生成器 …");
  const genDir = join(CACHE, version, "generated");
  const registriesPath = join(genDir, "reports", "registries.json");
  if (!existsSync(registriesPath)) {
    const java = findJava();
    execFileSync(java, ["-DbundlerMainClass=net.minecraft.data.Main", "-jar", jar, "--reports", "--output", genDir], {
      stdio: ["ignore", "ignore", "inherit"],
      env: { ...process.env, JAVA_TOOL_OPTIONS: "" },
      cwd: CACHE,
    });
  }
  const registries = JSON.parse(readFileSync(registriesPath, "utf8"));

  console.log("[4/5] 下载官方简体中文语言文件 …");
  const assetIndex = await getJson(meta.assetIndex.url);
  const langObj = assetIndex.objects["minecraft/lang/zh_cn.json"];
  if (!langObj) throw new Error("资源索引里没有 zh_cn.json");
  const langPath = join(CACHE, version, "zh_cn.json");
  const h = langObj.hash;
  await download(`https://resources.download.minecraft.net/${h.slice(0, 2)}/${h}`, langPath);
  const zh = JSON.parse(readFileSync(langPath, "utf8"));

  console.log("[5/5] 生成 catalog …");
  const itemIds = Object.keys(registries["minecraft:item"].entries).sort();
  const blockIds = Object.keys(registries["minecraft:block"].entries).sort();
  const entityIds = Object.keys(registries["minecraft:entity_type"]?.entries ?? {}).sort();
  const particleIds = Object.keys(registries["minecraft:particle_type"]?.entries ?? {}).sort();
  const blockSet = new Set(blockIds);

  const items = build(itemIds, zh, blockSet);
  const blocks = build(blockIds, zh, blockSet);
  const entities = buildEntities(entityIds, zh);
  const particles = buildParticles(particleIds);

  const stats = {};
  for (const [, , , cat] of items) stats[cat] = (stats[cat] || 0) + 1;

  const banner =
    `// 本文件由 scripts/gen-catalog.mjs 自动生成，请勿手工编辑。\n` +
    `// 数据来源：Minecraft ${version} 官方数据生成器 (registries.json) + 官方简体中文语言文件 zh_cn.json\n` +
    `// 重新生成：node scripts/gen-catalog.mjs ${version}\n\n`;

  writeFileSync(
    OUT,
    banner +
      `export const GENERATED_MC_VERSION = ${JSON.stringify(version)};\n\n` +
      `export const ITEMS = [\n${serialize(items)}\n] as const;\n\n` +
      `export const BLOCKS = [\n${serialize(blocks)}\n] as const;\n\n` +
      `export const ENTITIES = [\n${serialize(entities)}\n] as const;\n\n` +
      `export const PARTICLES = [\n${serialize(particles)}\n] as const;\n`,
  );

  console.log(
    `\n完成: ${items.length} 物品 / ${blocks.length} 方块 / ${entities.length} 实体 / ${particles.length} 粒子 -> ${OUT}`,
  );
  console.log("物品分类分布:", stats);
}

main().catch((err) => {
  console.error("\n生成失败:", err.message);
  process.exit(1);
});
