// 文档数据源：直接从编译器源码提取关键词、别名与注册表，避免手抄出错。
//
// 提取三部分：
// 1. `src/parser/keywords/` 的 `KEYWORDS` 常量与 `attributes!` 属性表；
// 2. 同一模块里全部 `"英文" | "中文" => ...` 形式的别名表（按函数分组）；
// 3. 游戏规则表、实体 NBT 别名及注册表和进度触发器快照。
//
// 文档工具与编译器共用同一份数据后，关键词对照表不可能与实现脱节；
// 示例翻译同样使用这些表，并由构建脚本调用真实编译器验证。

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";

const CJK = /[\u3400-\u9fff\uf900-\ufaff]/;

/** 读取 Rust 模块：`src/x.rs` 与同名目录下的全部 `.rs`，拼接成单一数据源文本。 */
async function readRustModule(repoRoot, relative) {
  const parts = [await readFile(path.join(repoRoot, `${relative}.rs`), "utf8")];
  const directory = path.join(repoRoot, relative);
  let entries = [];
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const files = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".rs"))
    .map((entry) => entry.name)
    .sort();
  for (const name of files) {
    parts.push(await readFile(path.join(directory, name), "utf8"));
  }
  return parts.join("\n");
}

/** 读取并解析全部文档数据。 */
export async function loadKeywordTables(repoRoot) {
  const keywordsSource = await readRustModule(repoRoot, "src/parser/keywords");
  const worldSource = await readRustModule(repoRoot, "src/compiler/validate/world");
  const lspSource = await readRustModule(repoRoot, "src/lsp/features");
  const advancementSource = await readFile(
    path.join(repoRoot, "data/version/26.3/advancement_triggers.json"),
    "utf8",
  );
  const entityNbtSource = await readRustModule(repoRoot, "src/version/entity_nbt");
  const registriesSource = await readFile(
    path.join(repoRoot, "data/version/26.3/registries.json"),
    "utf8",
  );

  const keywords = parseConstantBlock(keywordsSource, "KEYWORDS");
  const attributes = parseAttributes(keywordsSource);
  const tables = parseAliasTables(keywordsSource);
  return {
    keywords,
    attributes,
    tables,
    keywordDocs: parseDocTable(lspSource, "KEYWORD_DOCS"),
    attributeDocs: parseDocTable(lspSource, "ATTRIBUTE_DOCS"),
    gameRules: parseGameRules(worldSource),
    resourceKinds: parseRegistryKinds(registriesSource),
    advancementTriggers: parseAdvancementTriggers(advancementSource),
    nbtAliases: parseNbtAliases(entityNbtSource),
  };
}

/** 解析 LSP 悬停说明：`("namespace", "声明项目命名空间；…")`。 */
function parseDocTable(source, name) {
  const start = source.indexOf(`const ${name}`);
  const end = source.indexOf("];", start);
  if (start < 0 || end < 0) throw new Error(`features.rs 中找不到 ${name}`);
  const docs = {};
  const pattern = /\(\s*"([a-z_]+)",\s*"((?:[^"\\]|\\.)*)",?\s*\)/g;
  for (const match of source.slice(start, end).matchAll(pattern)) {
    docs[match[1]] = match[2].replace(/\\(.)/g, "$1");
  }
  return docs;
}

/** 解析 `Keyword { english: "x", chinese: "y" }` 常量块。 */
function parseConstantBlock(source, name) {
  const start = source.indexOf(`const ${name}`);
  if (start < 0) throw new Error(`keywords.rs 中找不到 const ${name}`);
  const open = source.indexOf("&[", start);
  const end = source.indexOf("\n];", open);
  if (open < 0 || end < 0) throw new Error(`const ${name} 的数组边界无法识别`);

  const pairs = [];
  const pattern = /english:\s*"([^"]+)"\s*,\s*chinese:\s*"([^"]+)"/g;
  for (const match of source.slice(open, end).matchAll(pattern)) {
    pairs.push({ en: match[1], zh: match[2] });
  }
  if (pairs.length === 0) throw new Error(`const ${name} 没有解析出任何条目`);
  return pairs;
}

/** 属性的英文、中文与 AST 值共用 `attributes!` 调用表。 */
function parseAttributes(source) {
  const block = source.match(/attributes!\s*\{([^}]+)\}/);
  if (!block) throw new Error("keywords.rs 中找不到 attributes! 属性表");
  const pairs = [...block[1].matchAll(/\w+\s*=>\s*"([^"]+)",\s*"([^"]+)"\s*;/g)]
    .map((match) => ({ en: match[1], zh: match[2] }));
  if (pairs.length === 0) throw new Error("attributes! 没有解析出任何条目");
  return pairs;
}

/** 解析别名函数：`"en" | "zh" => ...` 与 `matches!(value, "en" | "zh")`。 */
function parseAliasTables(source) {
  // 常量块里没有别名对，先挖掉，避免把 Keyword 条目混进按函数分组的表。
  const masked = source.replace(/const KEYWORDS[\s\S]*?\n\];/, "");

  const headers = [];
  const headerPattern = /^((?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z_][a-z0-9_]*))/gm;
  for (const match of masked.matchAll(headerPattern)) {
    headers.push({ name: match[2], bodyStart: match.index + match[1].length });
  }

  const tables = {};
  headers.forEach((header, index) => {
    const bodyEnd = index + 1 < headers.length ? headers[index + 1].bodyStart : masked.length;
    const body = masked.slice(header.bodyStart, bodyEnd);
    const pairs = [];
    for (const match of body.matchAll(/"[^"]+"(?:\s*\|\s*"[^"]+")+/g)) {
      const words = [...match[0].matchAll(/"([^"]+)"/g)].map((word) => word[1]);
      const english = words.filter((word) => /^[a-z0-9_]+$/.test(word));
      const chinese = words.filter((word) => CJK.test(word));
      if (chinese.length !== 1) continue;
      for (const en of english) pairs.push({ en, zh: chinese[0] });
    }
    if (pairs.length > 0) tables[header.name] = pairs;
  });
  return tables;
}

/** 解析 26.3 GameRules 注册表：名称、类型与整数范围。 */
function parseGameRules(source) {
  const start = source.indexOf("const GAME_RULES");
  const end = source.indexOf("\n];", start);
  if (start < 0 || end < 0) throw new Error("world.rs 中找不到 GAME_RULES");

  const rules = [];
  const pattern =
    /\(\s*"([a-z0-9_]+)",\s*GameRuleKind::(Bool|Integer\(\s*(-?\d+),\s*(i32::MAX|-?\d+)\s*\))\s*,?\s*\)/g;
  for (const match of source.slice(start, end).matchAll(pattern)) {
    const range =
      match[2] === "Bool" ? null : [Number(match[3]), match[4] === "i32::MAX" ? 2147483647 : Number(match[4])];
    rules.push({ name: match[1], type: match[2] === "Bool" ? "bool" : "int", range });
  }
  if (rules.length === 0) throw new Error("GAME_RULES 没有解析出任何规则");
  const multilineRules = [
    "fire_spread_radius_around_player",
    "max_block_modifications",
    "max_command_sequence_length",
    "players_nether_portal_creative_delay",
    "players_nether_portal_default_delay",
    "players_sleeping_percentage",
  ];
  const missing = multilineRules.filter((name) => !rules.some((rule) => rule.name === name));
  if (missing.length > 0) {
    throw new Error(`GAME_RULES 漏掉多行规则：${missing.join(", ")}`);
  }
  return rules;
}

/** 读取版本快照里的 `resource` 支持类型（由 xtask 从 26.3 源码生成）。 */
function parseRegistryKinds(source) {
  const snapshot = JSON.parse(source);
  const kinds = snapshot.resource_kinds;
  if (!Array.isArray(kinds) || kinds.length === 0) {
    throw new Error("registries.json 中找不到 resource_kinds");
  }
  return [...kinds].sort();
}

/** 读取编译器共用的 26.3 进度触发器快照。 */
function parseAdvancementTriggers(source) {
  const snapshot = JSON.parse(source);
  if (!snapshot.triggers || Array.isArray(snapshot.triggers) || typeof snapshot.triggers !== "object") {
    throw new Error("advancement_triggers.json 中找不到 triggers");
  }
  const triggers = Object.keys(snapshot.triggers).sort();
  if (triggers.length === 0) throw new Error("triggers 没有任何触发器");
  return triggers;
}

/** 解析 `src/version/entity_nbt.rs` 的 `CHINESE_ALIASES`（中文别名 → 英文键）。 */
function parseNbtAliases(source) {
  const start = source.indexOf("const CHINESE_ALIASES");
  const end = source.indexOf("\n];", start);
  if (start < 0 || end < 0) throw new Error("entity_nbt.rs 中找不到 CHINESE_ALIASES");
  const pairs = [];
  for (const match of source.slice(start, end).matchAll(/\("([^"]+)",\s*"([^"]+)"\)/g)) {
    pairs.push({ en: match[2], zh: match[1] });
  }
  if (pairs.length === 0) throw new Error("CHINESE_ALIASES 没有解析出任何条目");
  return pairs;
}
