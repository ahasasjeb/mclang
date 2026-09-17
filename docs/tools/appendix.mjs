// 附录表格：全部内容直接来自 keywords.mjs 从编译器源码提取的数据。
//
// `:::table kind=keywords` 生成语言关键词与函数属性；
// `:::table kind=aliases` 生成方法、属性与枚举别名；
// `:::table kind=game-rules` 生成 26.3 GameRules 注册表；
// `:::table kind=resource-kinds` 生成 `resource` 声明可用的资源类型；
// `:::table kind=advancement-triggers` 生成 26.3 进度触发器清单；
// `:::table kind=nbt-aliases` 生成实体 NBT 标签的中文别名。

import { escapeHtml, parseAttributes, renderInline } from "./markdown.mjs";

/** 附录说明文字里的行内代码与加粗同样按 Markdown 渲染。 */
const noteHooks = {
  codeSpan: (text) => `<code>${escapeHtml(text)}</code>`,
};
const renderNote = (text) => renderInline(text, noteHooks);

/** 别名表的中文标题与一句话说明，键是 keywords.rs 里的函数名。 */
const ALIAS_GROUPS = [
  ["query_property", "实体查询属性", "写在 `query … { }` 里的过滤器；`limit(1)` 的查询才能用于 `xp.query`。"],
  ["item_property", "item 过滤器属性", "`query` 的 `item(contents) { … }` 过滤器属性，`id` 必填。"],
  ["item_stack_property", "物品定义属性", "`item_stack(…) { … }` 支持的全部物品组件。"],
  ["function_tag_property", "函数标签属性", "`fn_tag` 声明体内的属性。"],
  ["slot_name", "物品槽名", "类型化物品查询目前只支持容器内容槽。"],
  ["resource_kind", "资源类型别名", "`resource` 声明可用的中文类型写法；其余类型写英文。"],
  ["self_method", "self 方法", "执行上下文要求：`@entity` 任意实体、`@non_player` 非玩家实体、`@player` 玩家。"],
  ["message_target", "消息目标", "`message.*` 的三种目标。"],
  ["effect_method", "effect 方法", "状态效果操作。"],
  ["xp_method", "xp 方法", "经验值操作；`query` 只能出现在表达式里。"],
  ["xp_kind", "xp 类型", "`xp` 语句与 `xp.query` 的第二个实参。"],
  ["stopwatch_method", "stopwatch 方法", "秒表操作；`query` 只能出现在表达式里。"],
  ["place_method", "place 方法", "`place.*` 的四种放置方式。"],
  ["forceload_method", "forceload 方法", "区块强制加载操作。"],
  ["time_method", "time 方法", "世界时钟操作；`query` 与 `query_gametime` 是表达式。"],
  ["weather_kind", "weather 方法", "天气类型。"],
  ["gamerule_method", "gamerule 方法", "游戏规则操作；`query` 是表达式。"],
  ["worldborder_method", "worldborder 方法", "世界边界操作；`get` 是表达式。"],
  ["locate_kind", "locate 方法", "定位结构、生物群系或兴趣点。"],
  [
    "execute_clause",
    "execute 修饰符",
    "结构化 `execute` 的修饰符子句；`facing` 同时有坐标与 `entity(查询)` 两种写法。",
  ],
  ["entity_relation", "execute on 关系", "`on(关系)` 的取值，对应原版的关系扩展。"],
  ["anchor_value", "实体锚点", "`anchored(锚点)` 与 `facing(entity(查询), 锚点)` 的取值。"],
  [
    "store_method",
    "store 方法",
    "`store.result/success` 写用户计分板或 Boss 栏，`store.data` 写 NBT。",
  ],
  ["store_data_type", "store.data 类型", "`store.data` 的数值类型实参。"],
  ["bossbar_field", "Boss 栏字段", "`store.result/success(bossbar, \"id\", 字段)` 的可写字段。"],
  [
    "advancement_method",
    "advancement 方法",
    "`advancement.grant/revoke(…)` 的五种作用范围；只有 `grant`/`授予` 可以带准则名。",
  ],
  ["advancement_property", "进度声明属性", "`advancement … { … }` 声明体里的属性。"],
  ["criterion_property", "进度准则属性", "`criterion … { … }` 里的触发器与条件。"],
  ["reward_property", "进度奖励属性", "`reward { … }` 里的函数、经验与资源引用。"],
  ["display_property", "进度展示属性", "`display { … }` 里的图标、标题与展示开关。"],
  ["advancement_frame", "进度框样式", "`frame = …;` 的取值。"],
  ["advancement_requirements", "进度完成策略", "`requirements = …;` 的取值。"],
  ["rarity_value", "物品稀有度", "`rarity = …;` 的取值。"],
  ["entity_sort", "实体查询排序", "`sort(…);` 的取值。"],
  ["boolean_word", "布尔值", "所有需要 true/false 的位置都可以写 真/假。"],
  ["time_unit", "时间单位", "写在数字之后：`20 t`、`1.5 s`、`1 d`。"],
  ["text_color", "文本颜色", "`message.*` 的可选颜色实参。"],
  ["sound_source", "声音分类", "`sound.self(声音, 分类)` 的第二个实参。"],
  ["set_block_mode", "set_block 模式", "`set_block(位置, 方块, 模式)` 的第三个实参。"],
  ["fill_mode", "fill 模式", "`fill(起点, 终点, 方块, 模式)` 的第四个实参。"],
  ["clone_filter", "clone 过滤方式", "`clone` 的方块过滤选项；`filtered` 后还要跟方块谓词。"],
  ["clone_mode", "clone 复制模式", "`clone` 的复制方式。"],
  ["template_rotation", "模板旋转", "`place.template` 的旋转参数。"],
  ["template_mirror", "模板镜像", "`place.template` 的镜像参数。"],
  ["clone_dimension", "clone 维度选项", "`clone` 的跨维度选项，参数是维度资源位置。"],
  ["strict_word", "strict 标志", "`clone`、`fill`、`set_block`、`place.template` 的严格模式标志。"],
];

export function renderAppendixTable(attributesText, data) {
  const { attributes } = parseAttributes(attributesText);
  const kind = attributes.get("kind");
  switch (kind) {
    case "keywords":
      return renderKeywordTables(data);
    case "aliases":
      return renderAliasTables(data);
    case "game-rules":
      return renderGameRules(data);
    case "resource-kinds":
      return renderResourceKinds(data);
    case "advancement-triggers":
      return renderAdvancementTriggers(data);
    case "nbt-aliases":
      return renderNbtAliases(data);
    default:
      throw new Error(`未知的附录表格 kind=${kind ?? ""}`);
  }
}

function renderKeywordTables(data) {
  let html = filterBar("keywords", "过滤关键词：输入英文、中文或用途");
  html += heading4("语言关键词", `KEYWORDS · ${data.keywords.length} 项`);
  html += pairTable(
    data.keywords.map((pair) => ({
      en: pair.en,
      zh: pair.zh,
      note: data.keywordDocs[pair.en] ?? "",
    })),
  );
  html += heading4("函数属性", `ATTRIBUTES · ${data.attributes.length} 项`);
  html += pairTable(
    data.attributes.map((pair) => ({
      en: `@${pair.en}`,
      zh: `@${pair.zh}`,
      note: data.attributeDocs[pair.en] ?? "",
    })),
  );
  return html;
}

function renderAliasTables(data) {
  let html = filterBar("aliases", "过滤方法、属性与枚举：输入英文或中文");
  for (const [key, title, noteText] of ALIAS_GROUPS) {
    const pairs = data.tables[key];
    if (!pairs || pairs.length === 0) {
      throw new Error(`别名表 ${key} 没有数据，检查 ALIAS_GROUPS`);
    }
    html += heading4(title, `${key} · ${pairs.length} 项`);
    html += `<p class="table-note">${renderNote(noteText)}</p>`;
    html += pairTable(pairs.map((pair) => ({ en: pair.en, zh: pair.zh, note: "" })));
  }
  return html;
}

function renderGameRules(data) {
  let html = filterBar("game-rules", "过滤规则：输入规则名");
  html += heading4("游戏规则", `GAME_RULES · ${data.gameRules.length} 条（26.3 注册表）`);
  html +=
    '<p class="table-note">' +
    renderNote(
      '`gamerule.set("规则", 值)` 的值类型与范围在编译期检查；`gamerule.query("规则")` 可作为表达式读取结果。`max_minecart_speed` 需要 `minecart_improvements` 特性，默认特性集下该规则的命令不可用。',
    ) +
    "</p>";
  html += '<div class="table-wrap"><table class="alias-table"><thead><tr><th>规则</th><th>类型</th><th>取值范围</th></tr></thead><tbody>';
  for (const rule of data.gameRules) {
    const range = rule.range ? `${rule.range[0]} 到 ${rule.range[1]}` : "true / false";
    html += `<tr><td><code>${escapeHtml(rule.name)}</code></td><td>${rule.type === "bool" ? "布尔" : "整数"}</td><td>${escapeHtml(range)}</td></tr>\n`;
  }
  html += "</tbody></table></div>\n";
  return html;
}

function renderResourceKinds(data) {
  let html = filterBar("resource-kinds", "过滤资源类型");
  html += heading4("JSON 资源类型", `resource_kinds · ${data.resourceKinds.length} 项`);
  html +=
    '<p class="table-note">' +
    renderNote(
      '`resource <类型> <名称> = "…";` 声明的资源会写到 `data/<命名空间>/<类型>/<名称>.json`；`worldgen/*` 这类带斜杠的类型要写成字符串名称。其中只有 `predicate` 可以在 `if predicate(名称)` 里引用；类型清单来自 26.3 版本快照 `data/version/26.3-rc-2/registries.json`（`cargo xtask generate-version-data` 生成）。',
    ) +
    "</p>";
  html += '<div class="table-wrap"><table class="alias-table"><tbody>';
  for (const kind of data.resourceKinds) {
    html += `<tr><td><code>${escapeHtml(kind)}</code></td></tr>\n`;
  }
  html += "</tbody></table></div>\n";
  return html;
}

function renderAdvancementTriggers(data) {
  let html = filterBar("advancement-triggers", "过滤触发器：输入名称");
  html += heading4(
    "进度触发器",
    `TRIGGERS · ${data.advancementTriggers.length} 项（26.3 CriteriaTriggers）`,
  );
  html +=
    '<p class="table-note">' +
    renderNote(
      "`criterion … { trigger = 名称; }` 可用的触发器，名字可以省略 `minecraft:` 前缀；`conditions` 的字段结构由各触发器定义，写触发器条件的原始 JSON。",
    ) +
    "</p>";
  html += '<div class="table-wrap"><table class="alias-table"><tbody>';
  for (const trigger of data.advancementTriggers) {
    html += `<tr><td><code>minecraft:${escapeHtml(trigger)}</code></td></tr>\n`;
  }
  html += "</tbody></table></div>\n";
  return html;
}

function renderNbtAliases(data) {
  let html = filterBar("nbt-aliases", "过滤标签：输入英文键或中文别名");
  html += heading4(
    "实体 NBT 中文别名",
    `CHINESE_ALIASES · ${data.nbtAliases.length} 项`,
  );
  html +=
    '<p class="table-note">' +
    renderNote(
      '实体 `nbt { … }` 语句与 `set_block`/`fill` 方块实体数据的顶层键可以用中文别名，解析期归一化为英文键，产物与英文写法逐字节一致；实体语句在 `spawn`/`each` 里还会按实体类型检查键是否存在与值类型。别名只覆盖常用标签，其余写英文键；物品 `custom_data` 的键是用户数据，不做替换。',
    ) +
    "</p>";
  html +=
    '<div class="table-wrap"><table class="alias-table"><thead><tr><th>中文</th><th>English</th></tr></thead><tbody>';
  for (const pair of data.nbtAliases) {
    html += `<tr><td><code>${escapeHtml(pair.zh)}</code></td><td><code>${escapeHtml(pair.en)}</code></td></tr>\n`;
  }
  html += "</tbody></table></div>\n";
  return html;
}

function heading4(title, detail) {  return `<h4>${escapeHtml(title)} <small>${escapeHtml(detail)}</small></h4>\n`;
}

function pairTable(pairs) {
  let html =
    '<div class="table-wrap"><table class="alias-table"><thead><tr><th>English</th><th>中文</th><th>说明</th></tr></thead><tbody>';
  for (const pair of pairs) {
    html += `<tr><td><code>${escapeHtml(pair.en)}</code></td><td><code>${escapeHtml(pair.zh)}</code></td><td>${pair.note ? renderNote(pair.note) : ""}</td></tr>\n`;
  }
  html += "</tbody></table></div>\n";
  return html;
}

function filterBar(name, placeholder) {
  return (
    `<div class="filter-bar" data-filter-root="${escapeHtml(name)}">` +
    `<input type="search" class="table-filter" placeholder="${escapeHtml(placeholder)}" aria-label="${escapeHtml(placeholder)}">` +
    `<span class="filter-count"></span></div>\n`
  );
}
