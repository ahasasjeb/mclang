// 文档内容渲染器：支持手册需要的 Markdown 子集，并接入双语代码钩子。
//
// 与常见 Markdown 的差异都在这里显式定义，避免引入依赖：
// - 标题支持 `## 文本 {#稳定锚点}`；
// - 代码块围栏信息串支持 `key=value` 与裸标志（`verify`、`strict`、`raw`）；
// - `:::tip|note|warn|danger|details 标题` 容器；
// - `:::generated example=… file=…` 由构建脚本替换为真实编译产物；
// - `:::table kind=…` 由构建脚本生成附录表格。
//
// 行内代码交给 `hooks.codeSpan`：文档构建据此生成中英双语 span。

const CJK = /[\u3400-\u9fff\uf900-\ufaff]/;

export function renderMarkdown(source, hooks) {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const headings = [];
  const state = { html: "", lines, index: 0, headings, hooks };
  renderBlocks(state, lines, hooks, 0, lines.length, headings);
  return { html: state.html, headings };
}

/** 渲染 [start, end) 行区间内的块级内容。 */
function renderBlocks(state, lines, hooks, start, end, headings) {
  let index = start;
  while (index < end) {
    const line = lines[index];
    if (!line.trim()) {
      index += 1;
      continue;
    }

    const fence = /^```(.*)$/.exec(line);
    if (fence) {
      const close = findClosingFence(lines, index + 1, end);
      const content = lines.slice(index + 1, close).join("\n");
      state.html += hooks.codeBlock(parseAttributes(fence[1]), content);
      index = close + 1;
      continue;
    }

    const container = /^:::\s*([a-zA-Z-]+)(?:\s+(.*?))?\s*$/.exec(line);
    if (container) {
      const close = findClosingContainer(lines, index + 1, end);
      const inner = lines.slice(index + 1, close);
      const title = container[2] ?? "";
      state.html += hooks.container(container[1], title, renderNested(inner, hooks));
      index = close + 1;
      continue;
    }

    const heading = /^(#{1,6})\s+(.*?)\s*$/.exec(line);
    if (heading) {
      const level = heading[1].length;
      const { text, id } = splitHeadingId(heading[2], headings.length + 1);
      const inline = hooks.inline(text, { heading: true });
      state.html += `<h${level} id="${id}">${inline}</h${level}>\n`;
      headings.push({ level, id, text: plainText(text) });
      index += 1;
      continue;
    }

    if (/^-{3,}\s*$/.test(line)) {
      state.html += "<hr>\n";
      index += 1;
      continue;
    }

    if (line.trimStart().startsWith("|")) {
      const rows = [];
      let cursor = index;
      while (cursor < end && lines[cursor].trimStart().startsWith("|")) {
        rows.push(lines[cursor].trim());
        cursor += 1;
      }
      state.html += renderTable(rows, hooks);
      index = cursor;
      continue;
    }

    if (/^\s*([-*]|\d+\.)\s+/.test(line)) {
      const close = findListEnd(lines, index, end);
      state.html += renderList(lines.slice(index, close), hooks);
      index = close;
      continue;
    }

    // 段落：连续的普通行，中文之间不加空格。
    const paragraph = [];
    let cursor = index;
    while (cursor < end) {
      const current = lines[cursor];
      if (!current.trim()) break;
      if (/^(```|:::)/.test(current)) break;
      if (/^(#{1,6})\s/.test(current)) break;
      if (/^\s*([-*]|\d+\.)\s+/.test(current)) break;
      if (current.trimStart().startsWith("|")) break;
      if (/^-{3,}\s*$/.test(current)) break;
      paragraph.push(current.trim());
      cursor += 1;
    }
    state.html += `<p>${hooks.inline(joinParagraph(paragraph))}</p>\n`;
    index = cursor;
  }
}

/** 容器内容递归渲染：为 `:::generated`、`:::table` 提供一个闭合的占位渲染。 */
function renderNested(lines, hooks) {
  const state = { html: "", lines, index: 0, headings: [] };
  renderBlocks(state, lines, hooks, 0, lines.length, []);
  return state.html;
}

function findClosingFence(lines, start, end) {
  for (let index = start; index < end; index += 1) {
    if (/^```\s*$/.test(lines[index])) return index;
  }
  throw new Error("代码块缺少结束的 ```");
}

function findClosingContainer(lines, start, end) {
  let depth = 0;
  for (let index = start; index < end; index += 1) {
    if (/^:::\s*[a-zA-Z-]+/.test(lines[index])) depth += 1;
    else if (/^:::\s*$/.test(lines[index])) {
      if (depth === 0) return index;
      depth -= 1;
    }
  }
  throw new Error("容器缺少结束的 :::");
}

function findListEnd(lines, start, end) {
  let index = start;
  while (index < end) {
    const line = lines[index];
    if (!line.trim()) {
      // 空行后仍属于列表的唯一情形是下一行继续缩进；这里一律结束，保持简单。
      break;
    }
    if (/^\s*([-*]|\d+\.)\s+/.test(line) || /^\s{2,}\S/.test(line)) {
      index += 1;
      continue;
    }
    break;
  }
  return index;
}

export function parseAttributes(info) {
  const attributes = new Map();
  const flags = new Set();
  const tokens = info.trim().split(/\s+/).filter(Boolean);
  let language = "text";
  let start = 0;
  if (tokens.length > 0 && /^[a-zA-Z0-9+#-]+$/.test(tokens[0])) {
    language = tokens[0];
    start = 1;
  }
  for (const token of tokens.slice(start)) {
    const match = /^([a-zA-Z-]+)=("([^"]*)"|'([^']*)'|(\S+))$/.exec(token);
    if (match) {
      attributes.set(match[1], match[3] ?? match[4] ?? match[5]);
    } else {
      flags.add(token.replace(/["']/g, ""));
    }
  }
  return { language, flags, attributes };
}

function splitHeadingId(text, ordinal) {
  const explicit = /\s*\{#([a-zA-Z0-9-_]+)\}\s*$/.exec(text);
  if (explicit) {
    return { text: text.slice(0, explicit.index), id: explicit[1] };
  }
  const slug = text
    .replace(/`([^`]*)`/g, "$1")
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
  return { text, id: slug ? `sec-${slug}` : `sec-${ordinal}` };
}

/** 去掉行内标记，供目录使用。 */
export function plainText(text) {
  return text
    .replace(/`([^`]*)`/g, "$1")
    .replace(/\*\*([^*]*)\*\*/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

function joinParagraph(lines) {
  let result = "";
  for (const line of lines) {
    if (!result) {
      result = line;
      continue;
    }
    const previous = result[result.length - 1];
    const next = line[0];
    result += CJK.test(previous) && CJK.test(next) ? line : ` ${line}`;
  }
  return result;
}

function renderTable(rows, hooks) {
  // 单元格里的 `\|` 是转义竖线（例如 `value \| max`），不能当列分隔符。
  const cells = (row) =>
    row
      .replace(/^\||\|$/g, "")
      .split(/(?<!\\)\|/)
      .map((cell) => cell.trim().replace(/\\\|/g, "|"));
  const header = cells(rows[0]);
  const body = rows.slice(1).filter((row) => !/^\|[\s:|-]+\|$/.test(row));
  let html = '<div class="table-wrap"><table>\n<thead><tr>';
  html += header.map((cell) => `<th>${hooks.inline(cell)}</th>`).join("");
  html += "</tr></thead>\n<tbody>\n";
  for (const row of body) {
    html += "<tr>";
    html += cells(row)
      .map((cell) => `<td>${hooks.inline(cell)}</td>`)
      .join("");
    html += "</tr>\n";
  }
  html += "</tbody></table></div>\n";
  return html;
}

function renderList(lines, hooks) {
  let html = "<ul>";
  for (const line of lines) {
    const item = /^\s*([-*]|\d+\.)\s+(.*)$/.exec(line);
    if (item) {
      html += `<li>${hooks.inline(item[2])}</li>`;
    } else if (html.endsWith("</li>")) {
      html = html.replace(/<\/li>$/, ` ${hooks.inline(line.trim())}</li>`);
    }
  }
  html += "</ul>";
  return html;
}

/** 行内解析：代码、加粗、链接与转义。 */
export function renderInline(text, hooks) {
  let html = "";
  let index = 0;
  while (index < text.length) {
    const character = text[index];
    if (character === "\\" && index + 1 < text.length) {
      html += escapeHtml(text[index + 1]);
      index += 2;
      continue;
    }
    if (character === "`") {
      const close = text.indexOf("`", index + 1);
      if (close !== -1) {
        html += hooks.codeSpan(text.slice(index + 1, close));
        index = close + 1;
        continue;
      }
    }
    if (text.startsWith("**", index)) {
      const close = text.indexOf("**", index + 2);
      if (close !== -1) {
        html += `<strong>${renderInline(text.slice(index + 2, close), hooks)}</strong>`;
        index = close + 2;
        continue;
      }
    }
    if (character === "[") {
      const match = /^\[([^\]]*)\]\(([^)]*)\)/.exec(text.slice(index));
      if (match) {
        html += `<a href="${escapeHtml(match[2])}">${renderInline(match[1], hooks)}</a>`;
        index += match[0].length;
        continue;
      }
    }
    html += escapeHtml(character);
    index += 1;
  }
  return html;
}

export function escapeHtml(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
