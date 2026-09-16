// 代码高亮：在构建期把 Mclang 源码渲染为带类名的 span，页面不需要运行时高亮。
//
// 分类依据仍是编译器源码提取出的词表，因此中英文关键词的着色完全一致：
// 关键词、函数属性、成员方法、声明属性和枚举值用不同颜色区分。

import { escapeHtml } from "./markdown.mjs";
import { tokenize } from "./translate.mjs";

export function createHighlighter(data) {
  const keywords = new Set();
  for (const pair of data.keywords) {
    keywords.add(pair.en);
    keywords.add(pair.zh);
  }
  // 属性只在 `@` 之后着色：`fn tick()` 里的 tick 是函数名，不是属性。
  const attributes = new Set();
  for (const pair of data.attributes) {
    attributes.add(pair.en);
    attributes.add(pair.zh);
  }
  const literals = new Set();
  for (const pairs of Object.values(data.tables)) {
    for (const pair of pairs) {
      literals.add(pair.en);
      literals.add(pair.zh);
    }
  }

  const highlight = (text, language) => {
    if (language === "mcl" || language === "mclang") {
      return highlightMcl(text, keywords, attributes, literals);
    }
    if (language === "mcfunction") {
      return highlightMcfunction(text);
    }
    return escapeHtml(text);
  };

  return { highlight };
}

function highlightMcl(text, keywords, attributes, literals) {
  const tokens = tokenize(text);
  let html = "";
  tokens.forEach((token, index) => {
    if (token.type === "ws" || token.type === "number") {
      html += escapeHtml(token.text);
      return;
    }
    if (token.type === "string") {
      html += `<span class="tok-str">${escapeHtml(token.text)}</span>`;
      return;
    }
    if (token.type === "comment") {
      html += `<span class="tok-com">${escapeHtml(token.text)}</span>`;
      return;
    }
    if (token.type === "punct") {
      if (token.text === "@") {
        html += '<span class="tok-attr">@</span>';
        return;
      }
      if (token.text === "#") {
        html += '<span class="tok-tag">#</span>';
        return;
      }
      html += escapeHtml(token.text);
      return;
    }

    const previous = previousSignificant(tokens, index);
    const next = nextSignificant(tokens, index);
    let className = "tok-ident";
    if (previous?.text === "@") className = "tok-attr";
    else if (previous?.text === "#") className = "tok-tag";
    else if (previous?.text === ".") className = "tok-method";
    else if (keywords.has(token.text)) className = "tok-kw";
    else if (literals.has(token.text)) className = "tok-lit";
    else if (next?.text === "(") className = "tok-fn";
    else if (attributes.has(token.text)) className = "tok-ident";
    html += `<span class="${className}">${escapeHtml(token.text)}</span>`;
  });
  return html;
}

/** 生成的 .mcfunction：只标记注释，命令文本本身保持原样。 */
function highlightMcfunction(text) {
  return text
    .split("\n")
    .map((line) =>
      line.trimStart().startsWith("#")
        ? `<span class="tok-com">${escapeHtml(line)}</span>`
        : escapeHtml(line),
    )
    .join("\n");
}

function previousSignificant(tokens, index) {
  for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
    if (tokens[cursor].type !== "ws") return tokens[cursor];
  }
  return null;
}

function nextSignificant(tokens, index) {
  for (let cursor = index + 1; cursor < tokens.length; cursor += 1) {
    if (tokens[cursor].type !== "ws") return tokens[cursor];
  }
  return null;
}
