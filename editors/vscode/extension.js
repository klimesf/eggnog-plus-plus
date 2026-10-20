// Completions and hovers for eggnog++, driven by stdlib.json.
// No language server: the list isn't type-aware, so after `.` every method is
// offered, labelled with the types it works on.

const vscode = require('vscode');
const path = require('path');
const fs = require('fs');

function loadCatalog(extensionPath) {
  return JSON.parse(fs.readFileSync(path.join(extensionPath, 'stdlib.json'), 'utf8'));
}

// Methods that share a name (e.g. `find` on Grid and on chains) become one entry.
function groupMethods(methods) {
  const byName = new Map();
  for (const m of methods) {
    if (!byName.has(m.name)) byName.set(m.name, []);
    byName.get(m.name).push(m);
  }
  return byName;
}

function methodDoc(variants) {
  const md = new vscode.MarkdownString();
  for (const v of variants) {
    md.appendCodeblock(v.sig, 'eggnog');
    md.appendMarkdown(`*on ${v.on}*: ${v.doc}\n\n`);
  }
  return md;
}

function simpleDoc(entry) {
  const md = new vscode.MarkdownString();
  md.appendCodeblock(entry.sig, 'eggnog');
  md.appendMarkdown(entry.doc);
  return md;
}

// In Markdown, only help inside ```egg blocks.
function inEggCode(document, position) {
  if (document.languageId === 'eggnog') return true;
  let inside = false;
  for (let line = 0; line < position.line; line++) {
    const text = document.lineAt(line).text.trim();
    if (!inside && text.startsWith('```egg')) inside = true;
    else if (inside && text.startsWith('```')) inside = false;
  }
  return inside;
}

function activate(context) {
  const catalog = loadCatalog(context.extensionPath);
  const methods = groupMethods(catalog.methods);
  const selector = [{ language: 'eggnog' }, { language: 'markdown' }];

  const methodItems = [...methods.entries()].map(([name, variants]) => {
    const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Method);
    item.detail = variants.map((v) => `${v.sig}   — ${v.on}`).join('\n');
    item.documentation = methodDoc(variants);
    const takesArgs = variants.some((v) => !/\(\)/.test(v.sig));
    item.insertText = new vscode.SnippetString(takesArgs ? `${name}($1)` : `${name}()`);
    if (takesArgs) item.command = { command: 'editor.action.triggerParameterHints', title: '' };
    return item;
  });

  const topItems = [
    ...catalog.functions.map((f) => {
      const item = new vscode.CompletionItem(f.name, vscode.CompletionItemKind.Function);
      item.detail = f.sig;
      item.documentation = simpleDoc(f);
      item.insertText = new vscode.SnippetString(`${f.name}($1)`);
      return item;
    }),
    ...catalog.constants.map((c) => {
      const item = new vscode.CompletionItem(c.name, vscode.CompletionItemKind.Constant);
      item.detail = c.sig;
      item.documentation = simpleDoc(c);
      return item;
    }),
    ...catalog.keywords.map((k) => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
  ];

  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      selector,
      {
        provideCompletionItems(document, position) {
          if (!inEggCode(document, position)) return undefined;
          const before = document.lineAt(position.line).text.slice(0, position.character);
          // After `.` (also at the start of a continuation line): methods.
          if (/\.\w*$/.test(before) && !/\.\.\w*$/.test(before)) return methodItems;
          return topItems;
        },
      },
      '.'
    ),
    vscode.languages.registerHoverProvider(selector, {
      provideHover(document, position) {
        if (!inEggCode(document, position)) return undefined;
        const range = document.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_]+)?/);
        if (!range) return undefined;
        const word = document.getText(range);
        const charBefore = range.start.character > 0 ? document.lineAt(position.line).text[range.start.character - 1] : '';
        if (charBefore === '.' && methods.has(word)) return new vscode.Hover(methodDoc(methods.get(word)), range);
        const entry = catalog.functions.find((f) => f.name === word) || catalog.constants.find((c) => c.name === word);
        return entry ? new vscode.Hover(simpleDoc(entry), range) : undefined;
      },
    })
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
