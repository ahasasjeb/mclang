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
//   （`1s`、`1.5d`），与词法器一致并入数字 token；时间单位要写成 `1 s`、`5 d`；
// - `nbt { ... }` 与 `block_state("…") { ... }` 内部是数据：前者是用户 NBT
//   （只翻译布尔字面量），后者是原版方块状态属性名与取值，整块保持原样。
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
  data: "data_method",
  数据操作: "data_method",
  item: "item_method",
  物品: "item_method",
};

/** 后面跟 `=` 或 `(` 时按属性表翻译的函数。 */
const PROPERTY_FAMILIES = [
  "objective_property",
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
  "text_style_property",
];

/** 只在后面跟 `(` 时按函数名翻译的表：`facing(...)` 是 execute 子句，
 * `facing = "south"` 却是方块状态属性，不能混用同一张表。 */
const CALL_FAMILIES = ["execute_clause"];
const COMMAND_RECEIVERS = new Set(["test", "tag", "attribute", "ride", "rotate", "team", "waypoint", "datapack", "recipe", "loot", "random", "title", "bossbar", "dialog", "posteffect"]);
const COMMAND_CALLS = new Set(["kill", "enchant", "damage", "spreadplayers", "spectate", "swing", "trigger", "gamemode", "defaultgamemode", "difficulty", "spawnpoint", "setworldspawn", "list", "reload", "teleport", "has", "equals", "matches", "particle", "stopsound", "msg", "teammsg"]);

/** 表达式内建函数 `count(查询)`、`compute(...)`：关键词表优先于同名的声明属性
 * （`count` 在物品属性里是“数量”，在表达式里是“计数”）。 */
const EXPRESSION_CALLS = new Set(["count", "compute"]);

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
  number_format: ["number_format_kind"],
  render_type: ["render_type"],
  click: ["click_action"],
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
  const tables = {
    ...data.tables,
    schedule_method: [{ en: "clear", zh: "清除" }],
  };
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
        if (!backward.has(pair.zh)) backward.set(pair.zh, pair.en);
      }
      indexes.set(family, { forward, backward });
    }
    return indexes.get(family);
  };

  const rewrite = (word, family, target) => {
    const { forward, backward } = indexOf(family);
    return target === "zh"
      ? forward.get(word) ?? (backward.has(word) ? word : undefined)
      : backward.get(word) ?? backward.get(forward.get(word));
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
    const root = canonicalWord(parts[0]) ?? parts[0];
    const family = RECEIVER_FAMILIES[parts[0]];
    return parts.map((part, index) => {
      if (index === 0) return root;
      let method;
      if (root === "scoreboard" && (parts.length > 2 || hasPair(part, "scoreboard_group") || rewrite(part, "ui_value", "en") === "players")) {
        method = index === 1
          ? rewrite(part, "scoreboard_group", "en") ?? rewrite(part, "ui_value", "en")
          : rewrite(part, "command_value", "en");
      } else if (index === 1 && family) {
        method = rewrite(part, family, "en");
      }
      return method ?? rewrite(part, "command_value", "en") ?? canonicalWord(part) ?? part;
    }).join(".");
  };

  const blockFamily = (tokens, opens, brace, parent) => {
    const previous = previousSignificant(tokens, brace);
    if (!previous) return null;
    if (previous.text === ")") {
      const open = opens[previous.index];
      const callee = open === undefined ? null : canonicalCallee(tokens, open);
      if (callee === "item_stack") return "item_stack_property";
      if (callee === "entity") {
        const name = previousSignificant(tokens, open);
        const equals = previousSignificant(tokens, name.index);
        const queryName = equals && previousSignificant(tokens, equals.index);
        const declaration = queryName && previousSignificant(tokens, queryName.index);
        return equals?.text === "=" && queryName?.type === "ident" &&
          canonicalWord(declaration?.text) === "query" ? "query_property" : null;
      }
      if (["text", "translate", "keybind", "object", "score", "selector", "nbt"].includes(callee)) return "text_style_property";
      return null;
    }
    let start = previous;
    let before;
    while ((before = previousSignificant(tokens, start.index)) && ![";", "{", "}"].includes(before.text)) start = before;
    if (parent === "advancement_property") {
      return { criterion: "criterion_property", reward: "reward_property", display: "display_property" }[
        rewrite(start.text, "advancement_property", "en")
      ] ?? null;
    }
    if (canonicalWord(start.text) === "export") start = nextSignificant(tokens, start.index);
    return { advancement: "advancement_property", objective: "objective_property", fn_tag: "function_tag_property" }[
      canonicalWord(start?.text)
    ] ?? null;
  };

  // 只跳过 execute 的完整修饰符；遇到 if 等条件头就停止，避免误认条件中的函数。
  const isExecuteModifier = (tokens, opens, index) => {
    let previous;
    while ((previous = previousSignificant(tokens, index))) {
      if (canonicalWord(previous.text) === "execute") return true;
      if (previous.text !== ")" || opens[previous.index] === undefined) return false;
      const name = previousSignificant(tokens, opens[previous.index]);
      if (!name || !hasPair(name.text, "execute_clause")) return false;
      index = name.index;
    }
    return false;
  };

  /** 每个标识符所在的最近调用 / 代码块框架。 */
  const computeFrames = (tokens, declared) => {
    const frames = new Array(tokens.length);
    const stack = [];
    const opens = [];
    tokens.forEach((token, index) => {
      frames[index] = { ...(stack.at(-1) ?? {}) };
      if (token.type === "ident") {
        frames[index].executeModifier = hasPair(token.text, "execute_clause") &&
          isExecuteModifier(tokens, opens, index);
      }
      if (token.type !== "punct") return;
      if (token.text === "(") {
        let callee = canonicalCallee(tokens, index);
        const name = previousSignificant(tokens, index);
        if (name) {
          const before = previousSignificant(tokens, name.index);
          const declaration = canonicalWord(before?.text) === "fn";
          const family = frames[name.index].propertyFamily;
          const property = family && hasPair(name.text, family) && ["{", ";", "}"].includes(before?.text);
          if (declaration || (declared.has(name.text) && before?.text !== "." &&
            !property && !frames[name.index].executeModifier)) callee = null;
        }
        stack.push({ delimiter: "(", open: index, callee, argument: 0,
          computeSource: rewrite(nextSignificant(tokens, index)?.text, "compute_source", "en") });
      } else if (token.text === "{") {
        stack.push({ delimiter: "{", open: index, propertyFamily: blockFamily(tokens, opens, index, stack.at(-1)?.propertyFamily) });
      } else if (token.text === "[") {
        stack.push({ delimiter: "[", open: index });
      } else if ([")", "}", "]"].includes(token.text)) {
        const expected = { ")": "(", "}": "{", "]": "[" }[token.text];
        if (stack.at(-1)?.delimiter === expected) opens[index] = stack.pop().open;
      } else if (token.text === "," && stack.at(-1)?.delimiter === "(") {
        stack.at(-1).argument += 1;
      }
    });
    return frames;
  };

  const declaredNames = (tokens) => {
    const names = new Set();
    const declarations = new Set(["score", "objective", "query", "item", "storage", "data_slot", "advancement", "fn_tag", "fn", "let", "for"]);
    for (const token of tokens) {
      if (token.type !== "ident") continue;
      const word = canonicalWord(token.text);
      const resource = word === "resource";
      if (!declarations.has(word) && !resource && !["criterion", "准则"].includes(token.text)) continue;
      let name = nextSignificant(tokens, token.index);
      if (resource && name) name = nextSignificant(tokens, name.index);
      if (name?.type !== "ident") continue;
      names.add(name.text);
      if (word !== "fn") continue;
      const open = nextSignificant(tokens, name.index);
      if (open?.text !== "(") continue;
      let cursor = nextSignificant(tokens, open.index);
      let parameterStart = true;
      while (cursor && ![")", "{", ";"].includes(cursor.text)) {
        if (parameterStart && cursor.type === "ident") names.add(cursor.text);
        parameterStart = cursor.text === ",";
        cursor = nextSignificant(tokens, cursor.index);
      }
    }
    return names;
  };

  const argumentValue = (frame, word, target) => {
    const { callee, argument, computeSource } = frame;
    if (callee === "bossbar.get" && argument === 1) {
      return rewrite(word, "command_value", target) ?? rewrite(word, "ui_value", target);
    }
    let family;
    if (callee === "on" && argument === 0) family = "entity_relation";
    if (["store.result", "store.success"].includes(callee) && argument === 2) family = "bossbar_field";
    if (callee === "compute") {
      if (argument === 0) family = "compute_source";
      else if (argument === (computeSource === "default" ? 1 : 2)) family = "compute_kind";
    }
    if (callee === "scoreboard.operation" && argument === 2) family = "score_operation";
    if (callee === "scoreboard.objectives.modify.rendertype" && argument === 1) family = "render_type";
    if (callee === "scoreboard.objectives.modify.numberformat" && argument === 1) family = "number_format_kind";
    if (["scoreboard.players.numberformat", "scoreboard.players.display.numberformat"].includes(callee) && argument === 2) family = "number_format_kind";
    return family ? rewrite(word, family, target) : null;
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
  const translateWord = (tokens, frames, index, target, relaxed, declared) => {
    const word = tokens[index].text;
    const previous = previousSignificant(tokens, index);
    const next = nextSignificant(tokens, index);
    const alias = (word, family) => declared.has(word) ? null : rewrite(word, family, target);

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
      if (canonicalWord(root?.text) === "scoreboard" && (root !== receiver || hasPair(word, "scoreboard_group") || rewrite(word, "ui_value", "en") === "players")) {
        return root === receiver
          ? rewrite(word, "scoreboard_group", target) ?? rewrite(word, "ui_value", target)
          : rewrite(word, "command_value", target);
      }
      const family = receiver ? RECEIVER_FAMILIES[receiver.text] : undefined;
      if (family) {
        const rewritten = rewrite(word, family, target);
        if (rewritten) return rewritten;
      }
      return lookupKeywords(word, target);
    }

    if (frames[index].propertyFamily && ["{", ";", "}"].includes(previous?.text)) {
      const property = rewrite(word, frames[index].propertyFamily, target);
      if (property) return property;
    }
    if (frames[index].executeModifier) return rewrite(word, "execute_clause", target);
    const argument = argumentValue(frames[index], word, target);
    if (argument) return argument;

    if (next?.text === "=") {
      for (const family of PROPERTY_FAMILIES) {
        const rewritten = alias(word, family);
        if (rewritten) return rewritten;
      }
    }

    // 函数调用位置：声明属性与 execute 子句都写在这。
    if (next?.text === "(") {
      const canonical = canonicalWord(word);
      if (canonical === "function") return lookupKeywords(word, target) ?? word;
      if (COMMAND_CALLS.has(canonical) || COMMAND_RECEIVERS.has(canonical) || EXPRESSION_CALLS.has(canonical)) {
        const rewritten = lookupKeywords(word, target) ?? alias(word, "command_value") ?? alias(word, "ui_value");
        if (rewritten) return rewritten;
      }
      for (const family of [...PROPERTY_FAMILIES, ...CALL_FAMILIES]) {
        const rewritten = alias(word, family);
        if (rewritten) return rewritten;
      }
    }

    // `属性 = 值` 里的枚举值优先于关键词表：`frame = 目标` 的“目标”是
    // 进度框样式，不是 objective 关键词。
    if (previous?.text === "=") {
      const propertyToken = previousSignificant(tokens, previous.index);
      const property = canonicalWord(propertyToken?.text);
      const syntaxProperty = frames[index].propertyFamily &&
        hasPair(propertyToken?.text, frames[index].propertyFamily) &&
        ["{", ";", "}"].includes(previousSignificant(tokens, propertyToken.index)?.text);
      for (const family of PROPERTY_VALUE_CONTEXTS[property] ?? []) {
        const rewritten = syntaxProperty ? rewrite(word, family, target) : alias(word, family);
        if (rewritten) return rewritten;
      }
    }

    // 枚举值只在取值位置翻译，避免命中同名标识符。
    const frame = frames[index].callee;
    if (frame) {
      if (COMMAND_RECEIVERS.has(frame.split(".")[0]) || COMMAND_CALLS.has(frame)) {
        const rewritten = alias(word, "command_value") ??
          (frame.startsWith("bossbar.") || frame.startsWith("title.") || frame === "particle" ? alias(word, "ui_value") : null) ??
          alias(word, "text_color");
        if (rewritten) return rewritten;
      }
      for (const family of CALL_VALUE_CONTEXTS[frame] ?? []) {
        const rewritten = alias(word, family);
        if (rewritten) return rewritten;
      }
    }
    // 属性只在 `@` 之后成立：`load`、`tick` 作为函数名时必须保持原样。
    const keyword = lookupKeywords(word, target);
    if (keyword) return keyword;
    const boolean = alias(word, "boolean_word");
    if (boolean) return boolean;
    const timeUnit = alias(word, "time_unit");
    if (timeUnit && nextToNumber(tokens, index)) return timeUnit;
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

  /** `block_state` 关键词的两种写法。 */
  const isBlockStateKeyword = (word) =>
    keywords.some((pair) => pair.en === "block_state" && (pair.en === word || pair.zh === word));

  /**
   * `block_state("minecraft:light") { level = "15"; }` 的属性名与取值是原版数据，
   * 与 `nbt` 字面量一样整块保持原样；只有 `block_state` 关键词本身参与翻译。
   */
  const blockStateBodyTokens = (tokens) => {
    const opaque = new Set();
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (token.type !== "ident" || !isBlockStateKeyword(token.text)) continue;
      const open = nextSignificant(tokens, index);
      if (!open || open.text !== "(") continue;
      let depth = 0;
      let close = null;
      for (let cursor = open.index + 1; cursor < tokens.length; cursor += 1) {
        if (tokens[cursor].text === "(") depth += 1;
        else if (tokens[cursor].text === ")") {
          if (depth === 0) {
            close = tokens[cursor];
            break;
          }
          depth -= 1;
        }
      }
      if (!close) continue;
      const brace = nextSignificant(tokens, close.index);
      if (!brace || brace.text !== "{") continue;
      let braces = 0;
      for (let cursor = brace.index; cursor < tokens.length; cursor += 1) {
        opaque.add(cursor);
        if (tokens[cursor].text === "{") braces += 1;
        else if (tokens[cursor].text === "}") {
          braces -= 1;
          if (braces === 0) break;
        }
      }
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
    const declared = new Set([...declaredNames(tokens), ...(options.declared ?? [])]);
    const frames = computeFrames(tokens, declared);
    const nbtOpaque = nbtBodyTokens(tokens);
    const blockStateOpaque = blockStateBodyTokens(tokens);
    const importOpaque = importNameTokens(tokens);
    const relaxed = options.relaxed === true;
    return tokens
      .map((token, index) => {
        if (token.type !== "ident") return token.text;
        if (importOpaque.has(index)) return token.text;
        // NBT 块内部只翻译布尔字面量（`真`/`假`），键名与字符串是用户数据。
        if (nbtOpaque.has(index)) {
          if (["=", ":"].includes(nextSignificant(tokens, index)?.text)) return token.text;
          return rewrite(token.text, "boolean_word", target) ?? token.text;
        }
        // 方块状态的属性名与取值是原版数据，整块保持原样。
        if (blockStateOpaque.has(index)) return token.text;
        return translateWord(tokens, frames, index, target, relaxed, declared) ?? token.text;
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
    declaredNames: (code) => declaredNames(tokenize(code)),
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
    if (!["ws", "comment"].includes(tokens[cursor].type)) return tokens[cursor];
  }
  return null;
}

function nextSignificant(tokens, index) {
  for (let cursor = index + 1; cursor < tokens.length; cursor += 1) {
    if (!["ws", "comment"].includes(tokens[cursor].type)) return tokens[cursor];
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
