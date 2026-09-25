#!/usr/bin/env bun
// 手册构建脚本。
//
// 流程：
//   1. 从编译器源码提取关键词与别名表（keywords.mjs）；
//   2. 扫描 content/manual.md 里带 `verify` 标记的完整示例，用真实编译器
//      分别编译英文与中文关键词版本，并要求两份数据包逐字节一致；
//   3. 把示例、生成产物、附录表格渲染进 index.html。
//
// 用法：
//   bun docs/tools/build.mjs              # 常规构建（需要 target/debug/mclang）
//   bun docs/tools/build.mjs --self-test  # 额外验证仓库全部 examples 的双语翻译
//   bun docs/tools/build.mjs --no-build   # 不自动 cargo build

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import { loadKeywordTables } from "./keywords.mjs";
import { buildTranslator, tokenize } from "./translate.mjs";
import { createHighlighter } from "./highlight.mjs";
import {
  escapeHtml,
  parseAttributes,
  renderInline,
  renderMarkdown,
} from "./markdown.mjs";
import { renderAppendixTable } from "./appendix.mjs";

const toolsDir = import.meta.dir;
const repoRoot = path.resolve(toolsDir, "..", "..");
const docsDir = path.resolve(toolsDir, "..");
const contentPath = path.join(docsDir, "content", "manual.md");
const outputPath = path.join(docsDir, "index.html");
const workDir = path.join(repoRoot, "build", "docs-verify");

const argv = new Set(process.argv.slice(2));
const selfTest = argv.has("--self-test");
const autoBuild = !argv.has("--no-build");

main();

async function main() {
  const data = await loadKeywordTables(repoRoot);
  const translator = buildTranslator(data);
  const highlighter = createHighlighter(data);
  const compiler = ensureCompiler();

  rmSync(workDir, { recursive: true, force: true });
  mkdirSync(workDir, { recursive: true });

  const stats = { examples: 0, files: 0, selfTestFiles: 0 };
  const packs = new Map();
  const markdown = readFileSync(contentPath, "utf8");
  const examples = scanVerifiedExamples(markdown, translator);
  const vocabulary = buildVocabulary(data);

  for (const example of examples) {
    assertNormalized(example, vocabulary, translator);
    const pack = verifyExample(compiler, example);
    packs.set(example.id, pack);
    stats.examples += 1;
    stats.files += pack.size;
    console.log(`  示例 ${example.id}：两种写法编译一致（${pack.size} 个文件）`);
  }

  if (selfTest) {
    stats.selfTestFiles = verifyRepositoryExamples(compiler, translator);
  }

  const hooks = createHooks({ data, translator, highlighter, packs });
  const { html, headings } = renderMarkdown(markdown, hooks);
  const page = composePage({ content: html, headings, data, stats, examples });
  writeFileSync(outputPath, page, "utf8");

  console.log(`已写出 ${path.relative(repoRoot, outputPath)}`);
  console.log(
    `验证：${stats.examples} 个完整示例、${stats.files} 个生成文件` +
      (selfTest ? `；仓库示例双语翻译往返 ${stats.selfTestFiles} 个文件` : ""),
  );
  console.log(
    "提示：示例验证结果由 target/debug/mclang 实际编译得出，产物在 build/docs-verify/ 下。",
  );
}

/** 确保有一个可用的编译器；默认自动 cargo build。 */
function ensureCompiler() {
  const candidates = ["target/debug/mclang.exe", "target/debug/mclang"].map((relative) =>
    path.join(repoRoot, relative),
  );
  if (autoBuild || !candidates.some(existsSync)) {
    console.log("构建编译器（cargo build）…");
    const result = run("cargo", ["build"], repoRoot);
    if (!result.success) {
      fail(`cargo build 失败：\n${result.stderr.toString()}`);
    }
  }
  const binary = candidates.find(existsSync);
  if (!binary) fail("cargo build 之后仍然找不到 target/debug/mclang");
  return binary;
}

/** 扫描全部带 `verify` 的 mcl 代码块。 */
function scanVerifiedExamples(markdown, translator) {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const examples = [];
  const seen = new Set();
  for (let index = 0; index < lines.length; index += 1) {
    const fence = /^```(.*)$/.exec(lines[index]);
    if (!fence) continue;
    const info = parseAttributes(fence[1]);
    let cursor = index + 1;
    const content = [];
    while (cursor < lines.length && !/^```\s*$/.test(lines[cursor])) {
      content.push(lines[cursor]);
      cursor += 1;
    }
    index = cursor;
    if (!info.flags.has("verify")) continue;
    const id = info.attributes.get("id");
    if (!id) fail("带 verify 的代码块必须提供 id=<示例名>");
    if (seen.has(id)) fail(`示例 id 重复：${id}`);
    seen.add(id);
    const source = content.join("\n");
    examples.push({
      id,
      language: info.language,
      raw: info.flags.has("raw"),
      source,
      en: translator.translate(source, "en"),
      zh: translator.translate(source, "zh"),
    });
  }
  return examples;
}

/** 编译并比较一个示例的两种关键词写法。 */
function verifyExample(compiler, example) {
  const base = path.join(workDir, "examples", example.id);
  const enFile = path.join(base, "en", "main.mcl");
  const zhFile = path.join(base, "zh", "main.mcl");
  writeSource(enFile, example.en);
  writeSource(zhFile, example.zh);

  checkSource(compiler, example, enFile, "英文关键词");
  checkSource(compiler, example, zhFile, "中文关键词");

  const enPack = path.join(base, "pack-en");
  const zhPack = path.join(base, "pack-zh");
  buildSource(compiler, example, enFile, enPack);
  buildSource(compiler, example, zhFile, zhPack);
  compareTrees(enPack, zhPack, example.id);

  const files = new Map();
  for (const relative of listFiles(enPack)) {
    files.set(relative, readFileSync(path.join(enPack, relative), "utf8"));
  }
  return files;
}

/** 归一化检查使用的词表：漏翻的词编译器不会报错，但页面会中英混杂。 */
function buildVocabulary(data) {
  const english = new Set(data.keywords.map((pair) => pair.en));
  const chinese = new Set(data.keywords.map((pair) => pair.zh));
  const englishAttributes = new Set(data.attributes.map((pair) => pair.en));
  const chineseAttributes = new Set(data.attributes.map((pair) => pair.zh));
  for (const pair of data.tables.boolean_word ?? []) {
    english.add(pair.en);
    chinese.add(pair.zh);
  }
  return { english, chinese, englishAttributes, chineseAttributes };
}

/** 检查示例的两种版本都不再残留另一种语言的关键词。 */
function assertNormalized(example, vocabulary, translator) {
  const check = (text, label, keywords, attributes) => {
    const tokens = tokenize(text);
    // NBT 字面量内部是用户数据：键名恰好与关键词同名时不做归一化要求。
    const opaque = translator.nbtBodyTokens(tokens);
    const leftovers = new Set();
    tokens.forEach((token, index) => {
      if (token.type !== "ident") return;
      if (opaque.has(index)) return;
      const previous = previousSignificant(tokens, index);
      if (previous?.text === "@") {
        if (attributes.has(token.text)) leftovers.add(`@${token.text}`);
        return;
      }
      if (keywords.has(token.text)) leftovers.add(token.text);
    });
    if (leftovers.size > 0) {
      fail(
        `示例 ${example.id} 的${label}没有完全归一化：${[...leftovers].join("、")}\n` +
          "（关键词漏翻不会导致编译失败，但页面上会中英混杂，请检查 docs/tools/translate.mjs）",
      );
    }
  };
  check(example.zh, "中文关键词版", vocabulary.english, vocabulary.englishAttributes);
  check(example.en, "英文关键词版", vocabulary.chinese, vocabulary.chineseAttributes);
}

function previousSignificant(tokens, index) {
  for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
    if (tokens[cursor].type !== "ws") return tokens[cursor];
  }
  return null;
}

function checkSource(compiler, example, file, label) {
  const args = ["check", file];
  if (!example.raw) args.push("--deny-raw");
  const result = run(compiler, args, repoRoot);
  if (!result.success) {
    fail(`示例 ${example.id}（${label}）检查失败：\n${result.stdout}${result.stderr}`);
  }
}

function buildSource(compiler, example, file, output) {
  const args = ["build", file, "-o", output, "--description", `Mclang 手册示例 ${example.id}`];
  if (!example.raw) args.push("--deny-raw");
  const result = run(compiler, args, repoRoot);
  if (!result.success) {
    fail(`示例 ${example.id} 构建失败：\n${result.stdout}${result.stderr}`);
  }
}

/** 复制 examples/ 下的真实项目，双语翻译后编译并比较，作为翻译器的回归测试。 */
function verifyRepositoryExamples(compiler, translator) {
  const examplesRoot = path.join(repoRoot, "examples");
  const projects = [];
  for (const entry of readdirSync(examplesRoot, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      projects.push({ name: entry.name, dir: path.join(examplesRoot, entry.name) });
    } else if (entry.name.endsWith(".mcl")) {
      projects.push({
        name: entry.name.replace(/\.mcl$/, ""),
        dir: examplesRoot,
        only: entry.name,
      });
    }
  }

  let checkedFiles = 0;
  for (const project of projects) {
    const sources = project.only
      ? [project.only]
      : listFiles(project.dir).filter((file) => file.endsWith(".mcl"));
    const enDir = path.join(workDir, "selftest", project.name, "en");
    const zhDir = path.join(workDir, "selftest", project.name, "zh");
    for (const relative of sources) {
      const text = readFileSync(path.join(project.dir, relative), "utf8");
      writeSource(path.join(enDir, relative), translator.translate(text, "en"));
      writeSource(path.join(zhDir, relative), translator.translate(text, "zh"));
      checkedFiles += 1;
    }
    // 单文件示例按“单模块项目”检查与构建；目录示例以 main.mcl 为入口。
    const enTarget = project.only ? path.join(enDir, project.only) : enDir;
    const zhTarget = project.only ? path.join(zhDir, project.only) : zhDir;
    for (const [label, target] of [
      ["英文", enTarget],
      ["中文", zhTarget],
    ]) {
      const result = run(compiler, ["check", target, "--deny-raw"], repoRoot);
      if (!result.success) {
        fail(`仓库示例 ${project.name}（${label}）检查失败：\n${result.stdout}${result.stderr}`);
      }
    }
    const enPack = path.join(workDir, "selftest", project.name, "pack-en");
    const zhPack = path.join(workDir, "selftest", project.name, "pack-zh");
    for (const [label, target, pack] of [
      ["英文", enTarget, enPack],
      ["中文", zhTarget, zhPack],
    ]) {
      const result = run(compiler, ["build", target, "-o", pack], repoRoot);
      if (!result.success) {
        fail(`仓库示例 ${project.name}（${label}）构建失败：\n${result.stdout}${result.stderr}`);
      }
    }
    compareTrees(enPack, zhPack, project.name);
    console.log(`  仓库示例 ${project.name}：双语翻译产物一致`);
  }
  return checkedFiles;
}

function writeSource(file, text) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text.endsWith("\n") ? text : `${text}\n`, "utf8");
}

/** 比较两棵目录树的相对路径集合与文件内容。 */
function compareTrees(left, right, label) {
  const leftFiles = listFiles(left);
  const rightFiles = listFiles(right);
  if (leftFiles.length !== rightFiles.length) {
    fail(
      `${label}：两种写法的产物文件数不同（${leftFiles.length} vs ${rightFiles.length}）`,
    );
  }
  for (let index = 0; index < leftFiles.length; index += 1) {
    if (leftFiles[index] !== rightFiles[index]) {
      fail(`${label}：产物路径不同（${leftFiles[index]} vs ${rightFiles[index]}）`);
    }
    const a = readFileSync(path.join(left, leftFiles[index]));
    const b = readFileSync(path.join(right, rightFiles[index]));
    if (!a.equals(b)) {
      fail(`${label}：产物内容不同（${leftFiles[index]}）`);
    }
  }
}

function listFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(full);
      else files.push(path.relative(root, full).split(path.sep).join("/"));
    }
  };
  walk(root);
  return files.sort();
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "buffer" });
  return {
    success: result.status === 0,
    stdout: result.stdout ?? Buffer.from(""),
    stderr: result.stderr ?? Buffer.from(""),
  };
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

/** 渲染钩子：代码块双语化、生成产物替换、附录表格生成。 */
function createHooks({ data, translator, highlighter, packs }) {
  const hooks = {
    inline(text) {
      return renderInline(text, hooks);
    },
    codeSpan(text) {
      // 以 `!` 开头的行内代码是“原样照排”：生成命令、选择器参数等不参与关键词切换。
      const raw = text.startsWith("!");
      const source = raw ? text.slice(1) : text;
      if (!raw && translator.isTranslatableInline(source)) {
        const en = translator.translateInline(source, "en");
        const zh = translator.translateInline(source, "zh");
        if (en !== zh) {
          return `<code class="kw"><span class="kw-en">${escapeHtml(en)}</span><span class="kw-zh">${escapeHtml(zh)}</span></code>`;
        }
      }
      return `<code>${escapeHtml(source)}</code>`;
    },
    codeBlock(info, content) {
      const title = info.attributes.get("title") ?? "";
      const verified = info.flags.has("verify");
      const language = info.language;
      const isMcl = language === "mcl" || language === "mclang";
      let enHtml;
      let zhHtml = null;
      if (isMcl) {
        enHtml = highlighter.highlight(translator.translate(content, "en"), "mcl");
        zhHtml = highlighter.highlight(translator.translate(content, "zh"), "mcl");
      } else {
        enHtml = highlighter.highlight(content, language);
      }
      const badges = [];
      if (verified) badges.push('<span class="badge ok">编译器验证通过</span>');
      if (info.flags.has("fragment")) badges.push('<span class="badge">片段</span>');
      return renderCodeFigure({ title, language, enHtml, zhHtml, badges });
    },
    container(type, title, inner) {
      if (type === "generated") {
        return renderGenerated(title, packs, highlighter);
      }
      if (type === "table") {
        return renderAppendixTable(title, data);
      }
      if (type === "details") {
        const summary = renderInline(title, hooks);
        return `<details class="faq"><summary>${summary}</summary>${inner}</details>\n`;
      }
      if (type === "lede") {
        return `<p class="lede">${renderInline(title, hooks)}${inner}</p>\n`;
      }
      const titleHtml = title ? `<p class="callout-title">${renderInline(title, hooks)}</p>` : "";
      return `<div class="callout ${escapeHtml(type)}">${titleHtml}${inner}</div>\n`;
    },
  };
  return hooks;
}

function renderCodeFigure({ title, language, enHtml, zhHtml, badges }) {
  const titleHtml = title ? `<span class="code-title">${escapeHtml(title)}</span>` : "";
  const badgeHtml = badges.join("");
  const dual = zhHtml !== null;
  const body = dual
    ? `<code class="kw-en">${enHtml}</code><code class="kw-zh">${zhHtml}</code>`
    : `<code>${enHtml}</code>`;
  return (
    `<figure class="code" data-lang="${escapeHtml(language)}"${dual ? ' data-dual="true"' : ""}>` +
    `<figcaption>${titleHtml}<span class="code-actions">${badgeHtml}` +
    `<button class="copy" type="button" title="复制当前显示的代码">复制</button></span></figcaption>` +
    `<pre>${body}</pre></figure>\n`
  );
}

/** `:::generated example=id file=包内路径`：替换为真实编译产物。 */
function renderGenerated(attributesText, packs, highlighter) {
  const attributes = parseAttributes(attributesText);
  const example = attributes.attributes.get("example");
  const file = attributes.attributes.get("file");
  if (!example || !file) fail(":::generated 需要 example=<示例> 与 file=<包内路径>");
  const pack = packs.get(example);
  if (!pack) fail(`:::generated 引用了未验证的示例 ${example}`);
  const content = pack.get(file);
  if (content === undefined) {
    fail(`示例 ${example} 的产物里没有 ${file}\n可用文件：\n${[...pack.keys()].join("\n")}`);
  }
  const title = attributes.attributes.get("title") ?? file;
  return renderCodeFigure({
    title,
    language: "mcfunction",
    enHtml: highlighter.highlight(content.replace(/\n$/, ""), "mcfunction"),
    zhHtml: null,
    badges: ['<span class="badge">编译器实际产物</span>'],
  });
}

/** 组装最终 HTML 页面。 */
function composePage({ content, headings, data, stats, examples }) {
  const version = /^version\s*=\s*"([^"]+)"/m.exec(
    readFileSync(path.join(repoRoot, "Cargo.toml"), "utf8"),
  )?.[1];
  const navigationHeadings = headings.filter(
    (heading) => heading.level === 2 || heading.level === 3,
  );
  const toc = navigationHeadings
    .map(
      (heading) =>
        `<a class="lvl-${heading.level}" href="#${heading.id}">${escapeHtml(heading.text)}</a>`,
    )
    .join("\n");
  const lead = /<p class="lede">([\s\S]*?)<\/p>\s*/.exec(content);
  if (!lead) fail("手册正文缺少 :::lede 首页简介");
  const articleContent = content.replace(lead[0], "");
  const verifiedList = examples
    .map((example) => `<code>${escapeHtml(example.id)}</code>`)
    .join("、");

  return `<!DOCTYPE html>
<html lang="zh-CN" data-kw="zh" data-theme="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#14251e">
<title>Mclang ${escapeHtml(version ?? "")} · Minecraft 26.3 数据包语言手册</title>
<meta name="description" content="Mclang ${version} 语言手册：语法、语义、执行上下文、编译产物、双语关键词与完整示例。">
<link rel="stylesheet" href="assets/manual.css">
<script>
// 在样式应用前恢复主题与关键词偏好，避免闪烁。
(function () {
  try {
    var theme = localStorage.getItem("mclang-doc-theme");
    if (theme === "dark" || theme === "light") document.documentElement.setAttribute("data-theme", theme);
    var keywords = localStorage.getItem("mclang-doc-keywords");
    if (keywords === "en" || keywords === "zh") document.documentElement.setAttribute("data-kw", keywords);
  } catch (error) { /* 隐私模式 */ }
})();
</script>
</head>
<body>
<a class="skip-link" href="#intro">跳到手册正文</a>
<header class="topbar">
  <button id="nav-toggle" type="button" aria-controls="toc" aria-expanded="false" aria-label="打开章节目录">目录</button>
  <a class="brand" href="#top" aria-label="Mclang 手册首页">
    <span class="brand-mark" aria-hidden="true">M</span>
    <span class="brand-name">Mclang <small>语言手册</small></span>
  </a>
  <nav class="top-links" aria-label="页面导航">
    <a href="#quickstart">快速开始</a>
    <a href="#appendix-keywords">关键词索引</a>
  </nav>
  <span class="spacer"></span>
  <span class="version-chip">v${escapeHtml(version ?? "")} <span>·</span> Java 26.3</span>
  <button id="keyword-toggle" type="button" title="切换代码块与行内关键词的语言">中文关键词</button>
  <button id="theme-toggle" type="button" title="切换深浅色">深色</button>
</header>
<div class="layout">
  <nav class="toc" id="toc" aria-label="目录">
    <div class="toc-title"><span>本手册</span><span class="toc-count">${navigationHeadings.length}</span></div>
    <label class="toc-search">
      <span class="search-icon" aria-hidden="true">⌕</span>
      <input id="toc-search" type="search" placeholder="搜索章节" aria-label="搜索手册章节" autocomplete="off">
      <kbd>/</kbd>
    </label>
    <div class="toc-links">
${toc}
    </div>
    <p id="toc-empty" class="toc-empty" hidden>没有找到匹配的章节</p>
    <a class="toc-source" href="#appendix-keywords">查看中英关键词对照 <span aria-hidden="true">↗</span></a>
  </nav>
  <main id="top">
    <section class="hero" aria-labelledby="welcome-title">
      <div class="hero-copy">
        <p class="eyebrow"><span class="status-dot"></span> LANGUAGE FOR MINECRAFT</p>
        <h1 id="welcome-title">用 MCL 写 Minecraft<br><span>数据包。</span></h1>
        <p class="hero-lede">${lead[1]}</p>
        <div class="hero-actions">
          <a class="button button-primary" href="#quickstart">开始使用 <span aria-hidden="true">↗</span></a>
          <a class="button button-secondary" href="#examples-core">浏览完整示例 <span aria-hidden="true">→</span></a>
        </div>
        <div class="hero-meta">
          <span><strong>${stats.examples}</strong> 个手册示例经编译器验证</span>
          <span class="hero-meta-divider" aria-hidden="true"></span>
          <span>中文与 English 关键词</span>
        </div>
      </div>
      <div class="hero-visual" role="img" aria-label="Mclang 将源码检查并编译为 Minecraft 数据包">
        <div class="visual-heading"><span>从源码到数据包</span><span class="visual-version">JAVA 26.3</span></div>
        <div class="flow-step flow-source">
          <span class="flow-number">01</span><span class="flow-icon">{ }</span>
          <span class="flow-copy"><strong>main.mcl</strong><small>结构化源码</small></span>
          <span class="flow-state">MCL</span>
        </div>
        <div class="flow-connector"><i></i></div>
        <div class="flow-step flow-compiler">
          <span class="flow-number">02</span><span class="flow-icon">✓</span>
          <span class="flow-copy"><strong>编译器检查</strong><small>语法 · 类型 · 资源</small></span>
          <span class="flow-state">OK</span>
        </div>
        <div class="flow-connector"><i></i></div>
        <div class="flow-step flow-pack">
          <span class="flow-number">03</span><span class="flow-icon">▤</span>
          <span class="flow-copy"><strong>数据包</strong><small>可放入世界的 datapacks/</small></span>
          <span class="flow-state">PACK</span>
        </div>
        <div class="visual-footer"><span>目标版本</span><strong>Minecraft Java Edition 26.3</strong></div>
      </div>
      <span class="hero-orbit hero-orbit-one" aria-hidden="true"></span>
      <span class="hero-orbit hero-orbit-two" aria-hidden="true"></span>
    </section>
${articleContent}
  </main>
</div>
<footer>
  <div class="wrap">
    <p>本手册由 <code>docs/tools/build.mjs</code> 依据编译器源码与真实编译结果生成：
    关键词与别名表取自 <code>src/parser/keywords.rs</code>，共 ${data.keywords.length} 个语言关键词、
    ${data.attributes.length} 个函数属性、${Object.keys(data.tables).length + 1} 组别名表。</p>
    <p>全部 ${stats.examples} 个标注“编译器验证通过”的完整示例（${verifiedList || "无"}）都分别用英文与中文关键词
    编译过，两种写法的数据包逐字节一致；仓库 <code>examples/</code> 示例的往返验证可用
    <code>bun docs/tools/build.mjs --self-test</code> 复现。</p>
    <p>目标版本：Minecraft Java Edition 26.3，数据包格式 121.0。手册随 <code>mclang ${escapeHtml(version ?? "")}</code> 生成。</p>
  </div>
</footer>
<button id="to-top" type="button" title="回到顶部">↑</button>
<script src="assets/manual.js"></script>
</body>
</html>
`;
}
