/*
 * Tests for the string tables.
 *
 * The point of this file is parity. A translation is not a feature that either works or
 * does not — it is a set of keys that silently rots as the interface changes. English gains
 * a key, the other tables do not, and the window shows an English sentence in the middle of
 * a Vietnamese one. Or a key is renamed in one table and the old spelling stays behind as
 * dead weight nobody notices.
 *
 * So rather than assert particular translations, these compare the tables against each
 * other and against how `logic.js` actually uses them.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { DEFAULT_LANGUAGE, LANGUAGES, TABLES, isSupportedLanguage, translator } from "./i18n.js";

const here = dirname(fileURLToPath(import.meta.url));
const read = (name) => readFileSync(join(here, name), "utf8");

const OTHERS = Object.keys(TABLES).filter((code) => code !== DEFAULT_LANGUAGE);

test("English is the reference table and is not empty", () => {
  assert.ok(TABLES[DEFAULT_LANGUAGE], "no default table");
  assert.ok(Object.keys(TABLES[DEFAULT_LANGUAGE]).length > 100, "the table looks truncated");
});

test("every language defines exactly the keys English defines", () => {
  const reference = Object.keys(TABLES[DEFAULT_LANGUAGE]).sort();

  for (const code of OTHERS) {
    const keys = Object.keys(TABLES[code]).sort();

    const missing = reference.filter((key) => !keys.includes(key));
    const extra = keys.filter((key) => !reference.includes(key));

    assert.deepEqual(missing, [], `${code} is missing keys: ${missing}`);
    assert.deepEqual(extra, [], `${code} has keys English does not: ${extra}`);
  }
});

test("no translation is left as the English string it was copied from", () => {
  // A handful legitimately match — a proper noun, a bare placeholder — so this checks that
  // the table was actually translated rather than duplicated, not that every value differs.
  for (const code of OTHERS) {
    const values = Object.entries(TABLES[code]);
    const identical = values.filter(([key, text]) => text === TABLES[DEFAULT_LANGUAGE][key]);

    assert.ok(
      identical.length < values.length * 0.2,
      `${code} is ${Math.round((identical.length / values.length) * 100)}% untranslated`,
    );
  }
});

test("no string is empty", () => {
  // An empty value passes a key-parity check and renders as a blank label.
  for (const [code, table] of Object.entries(TABLES)) {
    const blank = Object.entries(table)
      .filter(([, text]) => typeof text !== "string" || text.trim() === "")
      .map(([key]) => key);

    assert.deepEqual(blank, [], `${code} has blank strings: ${blank}`);
  }
});

test("every language carries the same placeholders in each string", () => {
  // `{remaining}` dropped from a translation leaves "còn  nữa" — grammatical-looking and
  // missing the number the sentence exists to deliver. A placeholder invented by a
  // translator is worse: it renders literally, braces and all.
  const placeholdersOf = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

  for (const code of OTHERS) {
    for (const [key, english] of Object.entries(TABLES[DEFAULT_LANGUAGE])) {
      assert.deepEqual(
        placeholdersOf(TABLES[code][key]),
        placeholdersOf(english),
        `${code} ${key}: placeholders differ`,
      );
    }
  }
});

test("every key the source asks for exists in the table", () => {
  // The other direction from key parity: parity keeps the tables equal to each other, this
  // keeps them equal to what the code looks up. A `t("jobs.emtpy")` typo would otherwise
  // reach the window as the key itself.
  const source = read("logic.js") + read("main.js");
  const looked = new Set(
    [...source.matchAll(/\bt\(\s*"([a-z][\w.]*)"/gi)].map((match) => match[1]),
  );

  assert.ok(looked.size > 20, `parsed only ${looked.size} lookups`);

  const unknown = [...looked].filter((key) => !(key in TABLES[DEFAULT_LANGUAGE])).sort();
  assert.deepEqual(unknown, [], `the source asks for keys no table has: ${unknown}`);
});

test("the consequence keys cover every capability Rust reports", () => {
  // These are looked up by interpolation (`consequence.${capability}`) rather than as
  // literals, so the test above cannot see them. The names come from the Rust enum.
  for (const capability of ["keep_awake", "power_off", "tray", "autostart", "notifications"]) {
    for (const code of Object.keys(TABLES)) {
      assert.ok(
        `consequence.${capability}` in TABLES[code],
        `${code} has no consequence for ${capability}`,
      );
    }
  }
});

test("the picker offers exactly the languages that have tables", () => {
  // An option with no table behind it selects a language that silently falls back to
  // English, which looks like the setting was ignored.
  assert.deepEqual(
    LANGUAGES.map((language) => language.code).sort(),
    Object.keys(TABLES).sort(),
  );
});

test("each language is named in its own language", () => {
  // A Vietnamese speaker looking for their language should not have to read English to
  // find it.
  const vietnamese = LANGUAGES.find((language) => language.code === "vi");
  assert.match(vietnamese.label, /Tiếng Việt/);
});

test("a translator returns the chosen language's string", () => {
  assert.notEqual(translator("vi")("nav.settings"), translator("en")("nav.settings"));
  assert.equal(translator("en")("nav.settings"), TABLES.en["nav.settings"]);
});

test("an unsupported language falls back rather than throwing", () => {
  // Reached if a stored language is removed in a later version. The window must still come
  // up, in English, rather than fail at boot.
  assert.equal(translator("kl")("nav.settings"), TABLES.en["nav.settings"]);
  assert.ok(!isSupportedLanguage("kl"));
  assert.ok(isSupportedLanguage("vi"));
});

test("placeholders are substituted, and an unknown one is left visible", () => {
  const t = translator("en");

  assert.match(t("dashboard.attentionMany", { count: 3 }), /3/);
  // Left as `{count}` rather than "undefined": a literal brace in the window reads as the
  // bug it is, where "undefined schedules" reads as a real message.
  assert.match(t("dashboard.attentionMany", {}), /\{count\}/);
});

test("the shutdown warning states the fact in every language", () => {
  // The one string where a translation choice could cost the user their work. Whatever the
  // wording, it has to name the machine turning off rather than hint at it.
  assert.match(TABLES.en["grace.heading"], /shutting down/i);
  assert.match(TABLES.vi["grace.heading"], /tắt/i);

  // And the cancel control has to read as the safe way out, not as another action.
  assert.match(TABLES.en["grace.cancel"], /cancel/i);
  assert.match(TABLES.vi["grace.cancel"], /hủy/i);
});
