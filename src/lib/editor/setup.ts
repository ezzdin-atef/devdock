// CodeMirror 6 for the Files tab: the basic setup (line numbers, folding, in-file
// search, bracket matching, history), word + language completion, a theme built on
// the app's CSS variables (so light and dark follow the app), and languages loaded
// on demand by file name.

import { basicSetup } from "codemirror";
import {
  autocompletion,
  completeAnyWord,
  completeFromList,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from "@codemirror/autocomplete";
import { indentWithTab } from "@codemirror/commands";
import { HighlightStyle, LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";

const theme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--text)",
    backgroundColor: "var(--bg-console)",
    fontSize: "13px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.6" },
  ".cm-content": { caretColor: "var(--accent-fg)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent-fg)" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection":
    { backgroundColor: "var(--accent-active)" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground": {
    backgroundColor: "color-mix(in srgb, var(--accent) 28%, transparent)",
  },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--bg-hover) 45%, transparent)" },
  ".cm-gutters": {
    backgroundColor: "var(--bg-console)",
    color: "var(--text-faint)",
    border: "none",
    borderRight: "1px solid var(--line)",
  },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--text-soft)" },
  ".cm-foldPlaceholder": {
    backgroundColor: "var(--bg-raised)",
    border: "1px solid var(--line-strong)",
    color: "var(--text-muted)",
  },
  ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
    backgroundColor: "var(--accent-tint)",
    outline: "1px solid var(--accent-panel-border)",
  },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)",
    outline: "1px solid var(--accent-panel-border)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "color-mix(in srgb, var(--accent) 45%, transparent)",
  },
  ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--info) 16%, transparent)" },
  ".cm-panels": {
    backgroundColor: "var(--bg-panel)",
    color: "var(--text)",
    fontFamily: "var(--font-sans)",
  },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--line)" },
  ".cm-panels.cm-panels-bottom": { borderTop: "1px solid var(--line)" },
  ".cm-panel input, .cm-panel button, .cm-textfield": {
    font: "inherit",
    fontSize: "12.5px",
    color: "var(--text)",
    backgroundColor: "var(--btn-bg)",
    border: "1px solid var(--btn-border)",
    borderRadius: "var(--radius-button-sm)",
  },
  ".cm-button": { backgroundImage: "none" },
  ".cm-tooltip": {
    backgroundColor: "var(--bg-overlay)",
    border: "1px solid var(--line-overlay)",
    borderRadius: "var(--radius-button)",
    boxShadow: "var(--shadow-menu)",
    color: "var(--text)",
  },
  ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--font-mono)", fontSize: "12.5px" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--accent-active)",
    color: "var(--text)",
  },
  ".cm-completionDetail": { color: "var(--text-muted)" },
  ".cm-completionMatchedText": { textDecoration: "none", color: "var(--accent-fg)", fontWeight: "600" },
});

// Syntax colours reuse the app's semantic tokens, which already have light and
// dark values with checked contrast.
const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.operatorKeyword], color: "var(--svc-violet)" },
  { tag: [t.string, t.special(t.string), t.regexp, t.character], color: "var(--success)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--svc-orange)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: "var(--info)" },
  { tag: [t.typeName, t.className, t.namespace, t.tagName], color: "var(--accent-fg)" },
  { tag: [t.definition(t.variableName), t.definition(t.propertyName)], color: "var(--text)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--text-soft)" },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: "var(--text-faint)", fontStyle: "italic" },
  { tag: [t.meta, t.processingInstruction, t.annotation], color: "var(--text-muted)" },
  { tag: t.heading, color: "var(--accent-fg)", fontWeight: "700" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.link, color: "var(--info)", textDecoration: "underline" },
  { tag: t.invalid, color: "var(--error)" },
]);

export const languageSlot = new Compartment();

/**
 * The language's own suggestions plus words already in the file, as one list.
 * Run separately they'd both offer the same names (a function shows up twice).
 */
async function wordsAndLanguage(ctx: CompletionContext): Promise<CompletionResult | null> {
  const fromLanguage: CompletionSource[] = ctx.state
    .languageDataAt<CompletionSource | readonly (string | Completion)[]>("autocomplete", ctx.pos)
    .map((s) => (typeof s === "function" ? s : completeFromList(s)));
  const results = (await Promise.all([...fromLanguage, completeAnyWord].map((src) => src(ctx)))).filter(
    (r): r is CompletionResult => r !== null && r.options.length > 0,
  );
  if (!results.length) return null;
  // Merge only results that replace the same text; language results come first,
  // so their richer entry (type icon, detail) wins over the plain word.
  const from = results[0].from;
  const seen = new Set<string>();
  const options: Completion[] = [];
  for (const r of results.filter((r) => r.from === from)) {
    for (const o of r.options) {
      if (seen.has(o.label)) continue;
      seen.add(o.label);
      options.push(o);
    }
  }
  return { from, options, validFor: /^[\w$]*$/ };
}

/** The language for `path`, loaded on demand; null for plain text. */
export async function languageFor(path: string): Promise<Extension> {
  const name = path.split("/").pop() ?? path;
  const desc = LanguageDescription.matchFilename(languages, name);
  if (!desc) return [];
  try {
    return await desc.load();
  } catch {
    return [];
  }
}

export interface Handlers {
  save: () => void;
  change: (state: EditorState) => void;
}

/** A fresh editor state for a file's text. */
export function createState(text: string, h: Handlers): EditorState {
  return EditorState.create({
    doc: text,
    extensions: [
      basicSetup,
      keymap.of([
        { key: "Mod-s", preventDefault: true, run: () => (h.save(), true) },
        indentWithTab,
      ]),
      autocompletion({ override: [wordsAndLanguage] }),
      languageSlot.of([]),
      theme,
      syntaxHighlighting(highlight),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) h.change(u.state);
      }),
    ],
  });
}
