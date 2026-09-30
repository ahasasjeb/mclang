import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { loadKeywordTables } from "./keywords.mjs";
import { buildTranslator } from "./translate.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const data = await loadKeywordTables(root);
const translator = buildTranslator(data);
const cases = JSON.parse(await readFile(new URL("../../tests/translate_cases.json", import.meta.url), "utf8"));

for (const { name, source, chinese } of cases) {
  test(name, () => {
    assert.equal(translator.translate(source, "zh"), chinese);
    assert.equal(translator.translate(chinese, "en"), source);
    assert.equal(translator.translate(source, "en"), source);
    assert.equal(translator.translate(chinese, "zh"), chinese);
  });
}

test("English compatibility spelling normalizes to a stable form", () => {
  const alias = 'scoreboard.objectives.modify.displayname(obj, "Points");';
  const canonical = 'scoreboard.objectives.modify.display_name(obj, "Points");';
  assert.equal(translator.translate(alias, "en"), canonical);
});

const contexts = [
  ["text_style_property", 'text("x") { PROBE = 1; }'],
  ["click_action", 'text("x") { click = PROBE("y"); }'],
  ["score_operation", "scoreboard.operation(self, obj, PROBE, self, other);"],
  ["data_method", 'data.PROBE(entity, self, "x");'],
  ["item_method", 'item.PROBE(entity, self, "weapon", with(reward));'],
  ["compute_kind", 'compute(default, PROBE, "minecraft:test");'],
  ["compute_source", "compute(PROBE);"],
  ["number_format_kind", "objective obj { number_format = PROBE; }"],
  ["number_format_kind", "scoreboard.objectives.modify.numberformat(obj, PROBE);"],
  ["render_type", "objective obj { render_type = PROBE; }"],
  ["render_type", "scoreboard.objectives.modify.rendertype(obj, PROBE);"],
];

test("every alias in the added families", () => {
  for (const [family, template] of contexts) {
    for (const { en, zh } of data.tables[family]) {
      for (const [source, expected, target] of [[en, zh, "zh"], [zh, en, "en"]]) {
        assert.equal(
          translator.translate(template.replace("PROBE", source), target),
          translator.translate(template, target).replace("PROBE", expected),
          `${family}: ${source}`,
        );
      }
    }
  }
});
