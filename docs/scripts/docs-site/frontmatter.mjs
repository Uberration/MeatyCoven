// Minimal front-matter parser backed by js-yaml 4.x.
//
// Replaces `gray-matter`, which (even at its latest 4.0.3) hard-binds the
// `safeLoad`/`safeDump` helpers that js-yaml removed in 4.x, pinning the docs
// build to js-yaml 3.x — the version flagged by GHSA-h67p-54hq-rp68 (quadratic
// DoS in merge-key handling). The docs build only ever calls `matter(raw)` and
// reads `{ data, content }`, so this thin shim covers the full surface we use
// while letting us depend on the patched js-yaml 4.2.0 directly.
//
// js-yaml 4's `load` is the old `safeLoad` (the unsafe loader was removed), so
// parsing semantics match gray-matter's previous behaviour.

import yaml from 'js-yaml';

// Opening fence: `---` on the first line (allowing trailing spaces + CRLF).
const OPEN = /^---[^\S\r\n]*\r?\n/;
// Closing fence: a `---` line, terminated by a newline or end of input.
const CLOSE = /\r?\n---[^\S\r\n]*(?:\r?\n|$)/;

export default function matter(input) {
  const str = typeof input === 'string' ? input : String(input ?? '');
  // Strip a leading UTF-8 BOM, matching gray-matter.
  const body = str.charCodeAt(0) === 0xfeff ? str.slice(1) : str;

  const open = OPEN.exec(body);
  if (!open) {
    return { data: {}, content: body };
  }

  const afterOpen = body.slice(open[0].length);
  const close = CLOSE.exec(afterOpen);
  if (!close) {
    // Unterminated front-matter: treat the whole input as content, like gray-matter.
    return { data: {}, content: body };
  }

  const rawYaml = afterOpen.slice(0, close.index);
  const content = afterOpen.slice(close.index + close[0].length);
  const data = rawYaml.trim() ? (yaml.load(rawYaml) ?? {}) : {};
  return { data, content };
}
