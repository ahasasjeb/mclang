// 中英关键词双向翻译：把一段 Mclang 源码改写为另一种关键词写法。
//
// 翻译只改语言关键词、函数属性、成员方法、声明属性和枚举值，
// 字符串、注释、数字、普通标识符与源码缩进原样保留。
//
// 关键词写法在两个方向上完全等价，因此算法是“把已识别的词改写成目标语言”：
// 输入本来用哪种语言并不重要，只改写当前不是目标语言的那些词。
//
// 依赖上下文的词不能按词表直译，这里按出现位置区分：
// - `@` 之后是函数属性（`entity` 既是关键词也是属性，`load` 也可以做函数名）；
// - `.` 之后是接收者的方法（`weather.clear` 是“晴朗”，`effect.clear` 是“清除”，
//   `self.item` 的 `item` 又回到关键词表）；
// - 后面跟着 `=` 或 `(` 的词是声明属性（`count = 1`、`limit(1)`）；
// - 枚举值只在明确的取值位置翻译：调用实参（`sort(nearest)`、`sound.self(id, master)`）
//   或 `属性 = 值`（`rarity = rare`）。否则 `player`、`master` 这类词表里的单词
//   会和普通标识符冲突；
// - `t`/`s`/`d` 在数字旁边是时间单位。其中 `s` 与 `d` 紧贴数字时是 NBT 后缀
//   （`1s`、`1.5d`），与词法器一致并入数字 token；时间单位要写成 `1 s`、`5 d`。
//
// 文档构建会用真实编译器同时编译两种写法并比较产物，任何翻译错误都会在
// 构建阶段以“无法编译”或“产物不一致”的形式暴露出来。

import { fileURLToPath } from "node:url";

const IDENT = /[\p{L}_][\p{L}\p{N}_]*/uy;
// 数字与 NBT 后缀一起扫描：`1b`、`2s`、`3i`、`4L`、`5.5f`、`6d`。后缀之后
// 不能紧跟标识符字符，否则按普通数字处理（`1bytes` = `1` + `bytes`）。
const NUMBER = /\d+(?:\.\d+)?(?:[bBsSiIlLfFdD](?![\p{L}\p{N}_]))?/y;
const WHITESPACE = /\s+/y;

/** 接收者 → 方法表；键同时包含英文与中文写法。 */
const RECEIVER_FAMILIES = {
  self: "self_method",
  自身: "self_method",
  message: "message_target",
  消息: "message_target",
  effect: "effect_method",
  效果: "effect_method",
  xp: "xp_method",
  经验: "xp_method",
  stopwatch: "stopwatch_method",
  秒表: "stopwatch_method",
  scoreboard: "scoreboard_method",
  计分板: "scoreboard_method",
  place: "place_method",
  放置: "place_method",
  forceload: "forceload_method",
  强制加载: "forceload_method",
  time: "time_method",
  时间: "time_method",
  gamerule: "gamerule_method",
  游戏规则: "gamerule_method",
  worldborder: "worldborder_method",
  世界边界: "worldborder_method",
  locate: "locate_kind",
  定位: "locate_kind",
  weather: "weather_kind",
  天气: "weather_kind",
  schedule: "schedule_method",
  调度: "schedule_method",
  advancement: "advancement_method",
  进度: "advancement_method",
  store: "store_method",
  存值: "store_method",
};

/** 方法表反查出的规范接收者，用于把中文接收者还原成英文。 */
const CANONICAL_RECEIVERS = {
  self_method: "self",
  message_target: "message",
  effect_method: "effect",
  xp_method: "xp",
  stopwatch_method: "stopwatch",
  scoreboard_method: "scoreboard",
  place_method: "place",
  forceload_method: "forceload",
  time_method: "time",
  gamerule_method: "gamerule",
  worldborder_method: "worldborder",
  locate_kind: "locate",
  weather_kind: "weather",
  schedule_method: "schedule",
  advancement_method: "advancement",
  store_method: "store",
};

/** 后面跟 `=` 或 `(` 时按属性表翻译的函数。 */
const PROPERTY_FAMILIES = [
  "query_property",
  "item_property",
  "item_stack_property",
  "function_tag_property",
  "slot_name",
  "resource_kind",
  "clone_dimension",
  "advancement_property",
  "criterion_property",
  "reward_property",
  "display_property",
];

/** 只在后面跟 `(` 时按函数名翻译的表：`facing(...)` 是 execute 子句，
 * `facing = "south"` 却是方块状态属性，不能混用同一张表。 */
const CALL_FAMILIES = ["execute_clause"];
const COMMAND_RECEIVERS = new Set(["tag", "attribute", "ride", "rotate", "team", "waypoint", "datapack", "recipe", "loot", "random", "title", "bossbar", "dialog", "posteffect"]);
const COMMAND_CALLS = new Set(["kill", "enchant", "damage", "spreadplayers", "spectate", "swing", "trigger", "gamemode", "defaultgamemode", "difficulty", "spawnpoint", "setworldspawn", "list", "reload", "teleport", "has", "equals", "matches", "particle", "stopsound", "msg", "teammsg"]);

/** 调用实参里允许出现的枚举值表，键是规范化的“接收者.方法”或裸函数名。 */
const CALL_VALUE_CONTEXTS = {
  sort: ["entity_sort"],
  item: ["slot_name"],
  "message.all": ["text_color"],
  "message.self": ["text_color"],
  "message.nearest": ["text_color"],
  "sound.self": ["sound_source"],
  stopsound: ["sound_source"],
  "xp.add": ["xp_kind"],
  "xp.set": ["xp_kind"],
  "xp.query": ["xp_kind"],
  clone: ["clone_filter", "clone_mode", "strict_word"],
  set_block: ["set_block_mode"],
  fill: ["fill_mode"],
  "place.template": ["template_rotation", "template_mirror", "strict_word"],
  anchored: ["anchor_value"],
  facing: ["anchor_value"],
  on: ["entity_relation"],
  "store.data": ["store_method", "store_data_type"],
  "store.result": ["bossbar_field"],
  "store.success": ["bossbar_field"],
};

/** `属性 = 值` 形式下的枚举值表。 */
const PROPERTY_VALUE_CONTEXTS = {
  rarity: ["rarity_value"],
  frame: ["advancement_frame"],
  requirements: ["advancement_requirements"],
};

/** 其余可以独立出现、但只在行内代码里放心的值表。 */
const LOOSE_VALUE_FAMILIES = [
  "entity_sort",
  "text_color",
  "sound_source",
  "rarity_value",
  "xp_kind",
  "set_block_mode",
  "fill_mode",
  "clone_filter",
  "clone_mode",
  "template_rotation",
  "template_mirror",
  "strict_word",
  "command_value",
];

export function buildTranslator(data) {
  // `schedule.clear` 的方法名在解析器里用 word_matches("clear") 单独判断，
  // keywords.rs 里没有对应的表，这里补上。
  const tables = { ...data.tables, schedule_method: [{ en: "clear", zh: "清除" }] };
  const keywords = data.keywords;
  const attributes = data.attributes;

  // 每个词表都建立双向索引，翻译时只查目标方向。
  const indexes = new Map();
  const indexOf = (family) => {
    if (!indexes.has(family)) {
      const forward = new Map();
      const backward = new Map();
      for (const pair of tables[family] ?? []) {
        forward.set(pair.en, pair.zh);
        backward.set(pair.zh, pair.en);
      }
      indexes.set(family, { forward, backward });
    }
    return indexes.get(family);
  };

  const rewrite = (word, family, target) => {
    const { forward, backward } = indexOf(family);
    return target === "zh" ? forward.get(word) : backward.get(word);
  };

  const hasPair = (word, family) => {
    const { forward, backward } = indexOf(family);
    return forward.has(word) || backward.has(word);
  };

  const isKeyword = (word) => keywords.some((pair) => pair.en === word || pair.zh === word);
  const isAttribute = (word) =>
    attributes.some((pair) => pair.en === word || pair.zh === word);
  const isPropertyWord = (word) =>
    [...PROPERTY_FAMILIES, ...CALL_FAMILIES].some((family) => hasPair(word, family));
  const isValueWord = (word) =>
    LOOSE_VALUE_FAMILIES.some((family) => hasPair(word, family)) ||
    hasPair(word, "boolean_word") ||
    hasPair(word, "time_unit") ||
    hasPair(word, "slot_name");

  const lookupKeywords = (word, target) =>
    target === "zh"
      ? (keywords.find((pair) => pair.en === word)?.zh ?? null)
      : (keywords.find((pair) => pair.zh === word)?.en ?? null);

  const lookupAttributes = (word, target) =>
    target === "zh"
      ? (attributes.find((pair) => pair.en === word)?.zh ?? null)
      : (attributes.find((pair) => pair.zh === word)?.en ?? null);

  /** 任意写法 → 英文规范词；用于还原中文接收者、方法与属性名。 */
  const canonicalWord = (word) => {
    if (!word) return null;
    const keyword = keywords.find((pair) => pair.en === word || pair.zh === word);
    if (keyword) return keyword.en;
    for (const family of [
      ...PROPERTY_FAMILIES,
      ...CALL_FAMILIES,
      ...Object.values(RECEIVER_FAMILIES),
      "command_value",
    ]) {
      const { forward, backward } = indexOf(family);
      if (forward.has(word)) return word;
      if (backward.has(word)) return backward.get(word);
    }
    return null;
  };

  /** `sound.self`、`message.nearest`、`sort` 这类被调用的名字。 */
  const calleeName = (tokens, openIndex) => {
    const method = previousSignificant(tokens, openIndex);
    if (!method || method.type !== "ident") return null;
    const parts = [method.text];
    let current = method;
    while (previousSignificant(tokens, current.index)?.text === ".") {
      const dot = previousSignificant(tokens, current.index);
      const receiver = previousSignificant(tokens, dot.index);
      if (receiver?.type !== "ident") break;
      parts.unshift(receiver.text);
      current = receiver;
    }
    return parts.join(".");
  };

  const canonicalCallee = (tokens, openIndex) => {
    const raw = calleeName(tokens, openIndex);
    if (!raw) return null;
    const parts = raw.split(".");
    if (COMMAND_RECEIVERS.has(canonicalWord(parts[0]) ?? parts[0])) {
      return parts.map(part => canonicalWord(part) ?? part).join(".");
    }
    const [head, tail] = raw.includes(".") ? raw.split(".") : [raw, null];
    const receiver = canonicalWord(head) ?? head;
    if (tail === null) return receiver;
    const family = RECEIVER_FAMILIES[receiver] ?? RECEIVER_FAMILIES[head];
    const canonicalReceiver = family ? CANONICAL_RECEIVERS[family] : receiver;
    const method =
      (family && rewrite(tail, family, "en")) ?? canonicalWord(tail) ?? tail;
    return `${canonicalReceiver}.${method}`;
  };

  /** 每个标识符所在的最近调用 / 代码块框架。 */
  const computeFrames = (tokens) => {
    const frames = new Array(tokens.length).fill(null);
    const stack = [];
    tokens.forEach((token, index) => {
      if (token.text === "(") {
        stack.push(canonicalCallee(tokens, index));
      } else if (token.text === ")") {
        stack.pop();
      } else if (token.text === "{") {
        stack.push("{");
      } else if (token.text === "}") {
        stack.pop();
      }
      frames[index] = stack[stack.length - 1] ?? null;
    });
    return frames;
  };

  /** `t`/`s`/`d` 前面是数字，或前面是逗号且逗号前面是数字（`time.set(6000, t)`）。 */
  const nextToNumber = (tokens, index) => {
    const previous = previousSignificant(tokens, index);
    if (!previous) return false;
    if (previous.type === "number") return true;
    if (previous.text === ",") {
      const before = previousSignificant(tokens, previous.index);
      return before?.type === "number";
    }
    return false;
  };

  /** 单个标识符的改写；返回 `null` 表示保持原样。 */
  const translateWord = (tokens, frames, index, target, relaxed) => {
    const word = tokens[index].text;
    const previous = previousSignificant(tokens, index);
    const next = nextSignificant(tokens, index);

    if (previous?.text === "@") return lookupAttributes(word, target);
    if (previous?.text === "#") return null;

    if (previous?.text === ".") {
      const receiver = previousSignificant(tokens, previous.index);
      let root = receiver;
      while (root && previousSignificant(tokens, root.index)?.text === ".") {
        const dot = previousSignificant(tokens, root.index);
        root = previousSignificant(tokens, dot.index);
      }
      if (COMMAND_RECEIVERS.has(canonicalWord(root?.text))) {
        return rewrite(word, "command_value", target) ?? rewrite(word, "ui_value", target) ?? lookupKeywords(word, target);
      }
      const family = receiver ? RECEIVER_FAMILIES[receiver.text] : undefined;
      if (family) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
      return lookupKeywords(word, target);
    }

    if (next?.text === "=") {
      for (const family of PROPERTY_FAMILIES) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
    }

    // 函数调用位置：声明属性与 execute 子句都写在这。
    if (next?.text === "(") {
      const canonical = canonicalWord(word);
      if (COMMAND_CALLS.has(canonical) || COMMAND_RECEIVERS.has(canonical)) {
        const rewritten = lookupKeywords(word, target) ?? rewrite(word, "command_value", target) ?? rewrite(word, "ui_value", target);
        if (rewritten) return rewritten;
      }
      for (const family of [...PROPERTY_FAMILIES, ...CALL_FAMILIES]) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
    }

    // `属性 = 值` 里的枚举值优先于关键词表：`frame = 目标` 的“目标”是
    // 进度框样式，不是 objective 关键词。
    if (previous?.text === "=") {
      const property = canonicalWord(previousSignificant(tokens, previous.index)?.text);
      for (const family of PROPERTY_VALUE_CONTEXTS[property] ?? []) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
    }

    // 属性只在 `@` 之后成立：`load`、`tick` 作为函数名时必须保持原样。
    const keyword = lookupKeywords(word, target);
    if (keyword) return keyword;

    const boolean = rewrite(word, "boolean_word", target);
    if (boolean) return boolean;

    const timeUnit = rewrite(word, "time_unit", target);
    if (timeUnit && nextToNumber(tokens, index)) return timeUnit;

    // 枚举值只在取值位置翻译，避免命中同名标识符。
    const frame = frames[index];
    if (frame && frame !== "{") {
      if (COMMAND_RECEIVERS.has(frame.split(".")[0]) || COMMAND_CALLS.has(frame)) {
        const rewritten = rewrite(word, "command_value", target) ??
          (frame.startsWith("bossbar.") || frame.startsWith("title.") || frame === "particle" ? rewrite(word, "ui_value", target) : null) ??
          rewrite(word, "text_color", target);
        if (rewritten) return rewritten;
      }
      for (const family of CALL_VALUE_CONTEXTS[frame] ?? []) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
    }
    if (relaxed) {
      for (const family of LOOSE_VALUE_FAMILIES) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
      const slot = rewrite(word, "slot_name", target);
      if (slot) return slot;
    }
    return null;
  };

  /** `nbt` / `数据`：进入 NBT 字面量的关键词。 */
  const isNbtKeyword = (word) =>
    keywords.some((pair) => pair.en === "nbt" && (pair.en === word || pair.zh === word));

  /**
   * NBT 字面量内部的 token 索引：`nbt { ... }` 里除开头的 `nbt` 之外全部保持原样。
   * 键名与字符串是用户数据，恰好叫 `count`、`data` 之类的属性名时不能被翻译。
   */
  const nbtBodyTokens = (tokens) => {
    const opaque = new Set();
    let depth = 0;
    let pending = false;
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (depth > 0) {
        opaque.add(index);
        if (token.text === "{") depth += 1;
        else if (token.text === "}") depth -= 1;
        continue;
      }
      if (token.type === "ws" || token.type === "comment") continue;
      if (pending && token.text === "{") {
        depth = 1;
        opaque.add(index);
        pending = false;
        continue;
      }
      pending = token.type === "ident" && isNbtKeyword(token.text);
    }
    return opaque;
  };

  // Import paths and exported names are identifiers, even when they spell a
  // keyword such as `time` or `random`. Only `import` and alias `as` are syntax.
  const importNameTokens = (tokens) => {
    const opaque = new Set();
    let inImport = false;
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (!inImport) {
        if (token.type === "ident" && canonicalWord(token.text) === "import") {
          inImport = true;
        }
        continue;
      }
      if (token.text === ";") {
        inImport = false;
      } else if (token.type === "ident" && canonicalWord(token.text) !== "as") {
        opaque.add(index);
      }
    }
    return opaque;
  };

  /** 翻译一段代码；`relaxed` 供正文行内代码使用（时间单位等不要求上下文）。 */
  const translate = (code, target, options = {}) => {
    const tokens = tokenize(code);
    const frames = computeFrames(tokens);
    const nbtOpaque = nbtBodyTokens(tokens);
    const importOpaque = importNameTokens(tokens);
    const relaxed = options.relaxed === true;
    return tokens
      .map((token, index) => {
        if (token.type !== "ident") return token.text;
        if (importOpaque.has(index)) return token.text;
        // NBT 块内部只翻译布尔字面量（`真`/`假`），键名与字符串是用户数据。
        if (nbtOpaque.has(index)) {
          return rewrite(token.text, "boolean_word", target) ?? token.text;
        }
        return translateWord(tokens, frames, index, target, relaxed) ?? token.text;
      })
      .join("");
  };

  /** 正文行内代码的标识符是否全部可识别：`#tag`、生成命令等一律保持原状。 */
  const isTranslatableInline = (text) => {
    const tokens = tokenize(text);
    let idents = 0;
    let translatable = true;
    tokens.forEach((token, index) => {
      if (token.type !== "ident") return;
      idents += 1;
      const previous = previousSignificant(tokens, index);
      const next = nextSignificant(tokens, index);
      // `<players>` 这类尖括号占位符不算标识符，不影响整段是否可翻译。
      if (previous?.text === "<" && next?.text === ">") return;
      if (previous?.text === "#") {
        translatable = false;
      } else if (previous?.text === "@") {
        if (!isAttribute(token.text) && !isKeyword(token.text)) translatable = false;
      } else if (next?.text === "=" || next?.text === "(") {
        if (!isPropertyWord(token.text) && !isKeyword(token.text) && !isValueWord(token.text)) {
          translatable = false;
        }
      } else if (!isKeyword(token.text) && !isValueWord(token.text)) {
        translatable = false;
      }
    });
    return idents > 0 && translatable;
  };

  const translateInline = (text, target) => translate(text, target, { relaxed: true });

  return {
    translate,
    translateInline,
    isTranslatableInline,
    canonicalCallee,
    nbtBodyTokens,
    isKnownWord: (word) => isKeyword(word) || isAttribute(word) || isValueWord(word) || isPropertyWord(word),
  };
}

/** 词法扫描：保留空白，逐字符切分。 */
export function tokenize(code) {
  const tokens = [];
  let index = 0;
  while (index < code.length) {
    if (code.startsWith('"""', index)) {
      const end = code.indexOf('"""', index + 3);
      const stop = end === -1 ? code.length : end + 3;
      tokens.push({ type: "string", text: code.slice(index, stop), index: tokens.length });
      index = stop;
      continue;
    }
    const character = code[index];
    if (character === '"') {
      let stop = index + 1;
      while (stop < code.length && code[stop] !== '"' && code[stop] !== "\n") {
        if (code[stop] === "\\") stop += 1;
        stop += 1;
      }
      if (code[stop] === '"') stop += 1;
      tokens.push({ type: "string", text: code.slice(index, stop), index: tokens.length });
      index = stop;
      continue;
    }
    if (character === "/" && code[index + 1] === "/") {
      let stop = index;
      while (stop < code.length && code[stop] !== "\n") stop += 1;
      tokens.push({ type: "comment", text: code.slice(index, stop), index: tokens.length });
      index = stop;
      continue;
    }
    const whitespace = matchAt(WHITESPACE, code, index);
    if (whitespace) {
      tokens.push({ type: "ws", text: whitespace, index: tokens.length });
      index += whitespace.length;
      continue;
    }
    const number = matchAt(NUMBER, code, index);
    if (number) {
      tokens.push({ type: "number", text: number, index: tokens.length });
      index += number.length;
      continue;
    }
    const ident = matchAt(IDENT, code, index);
    if (ident) {
      tokens.push({ type: "ident", text: ident, index: tokens.length });
      index += ident.length;
      continue;
    }
    tokens.push({ type: "punct", text: character, index: tokens.length });
    index += 1;
  }
  return tokens;
}

function matchAt(pattern, text, index) {
  pattern.lastIndex = index;
  const match = pattern.exec(text);
  return match && match.index === index ? match[0] : null;
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

// 命令行用法：`bun docs/tools/translate.mjs zh < file.mcl`，便于人工核对翻译结果。
if (import.meta.main) {
  const { loadKeywordTables } = await import("./keywords.mjs");
  const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
  const data = await loadKeywordTables(repoRoot);
  const translator = buildTranslator(data);
  const target = process.argv[2] === "en" ? "en" : "zh";
  const source = await new Response(Bun.stdin.stream()).text();
  process.stdout.write(translator.translate(source, target) + "\n");
}
