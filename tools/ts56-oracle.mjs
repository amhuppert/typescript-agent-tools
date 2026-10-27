// TypeScript 5.6.3 oracle: quick info, references, and module specifiers
// from the pinned 5.6.3 language service, for the TypeScript 5.6 dialect's
// acceptance tests.
//
//   [TYPE_SCRIPT_56_API=/path/to/typescript-5.6.3/package] \
//     node tools/ts56-oracle.mjs TSCONFIG QUERY...
//
// Each QUERY is two arguments, `type FILE:LINE:COLUMN`, `references
// FILE:LINE:COLUMN`, or `specifier IMPORTER=TARGET`; `ts56Oracle` takes the
// same queries as strings, such as 'type src/a.ts:1:14'. FILE, IMPORTER, and
// TARGET are relative to the configuration's directory or absolute; FILE may
// hold colons, and a specifier's operand is split at its first `=`. LINE and COLUMN are one-based, COLUMN in
// UTF-16 code units like tsparse's `--at`; lines break where 5.6.3's line map
// breaks them (LF, CR, CRLF, U+2028, U+2029). A position may be at most the
// end of its line.
//
// The package comes from TYPE_SCRIPT_56_API or the canonical checkout (see
// `ts56-compiler.mjs`). The language service runs over a host built
// from the configuration as 5.6.3 parses it, `incremental` forced false; it
// reads the project and writes nothing.
//
// It prints one JSON document, `{ config, version, results }`, with a result
// per query in order:
//   type        { query, quickInfo }: `getQuickInfoAtPosition`, as `text`
//               (`displayPartsToString` of its display parts) and the `span`
//               it covers, `{ start, end }` one-based with an exclusive end,
//               in the query's file; null where 5.6.3 has no quick info.
//   references  { query, references }: `findReferences`, each referenced
//               symbol's references in turn, as `{ file, line, column,
//               definition, write }` at the reference's start; `write` is
//               TypeScript's `isWriteAccess` and `definition` its
//               `isDefinition`, which marks the declarations of the symbol at
//               the position only when that position is itself a declaration's
//               name; null where 5.6.3 finds no symbol.
//   specifier   { query, specifiers }: `moduleSpecifiers.getModuleSpecifiers`
//               of TARGET's module from IMPORTER with the preferences the
//               checker's `getSpecifierForModuleSymbol` gives it (project-
//               relative, and the `js` ending when IMPORTER's default mode is
//               ESNext); an import of TARGET that IMPORTER writes is reused,
//               as the checker reuses it; null where TARGET is no module. A
//               collision question 5.6.3 asks names a file relative to this
//               process's working directory, as the compiler's host does.
// Files are named as `program-inputs.mjs` names them: a standard library from
// the 5.6.3 package by its file name, any other file by its canonical path
// relative to the configuration's directory.
//
// Exits 0 with the document; 1 on a bad query (an unknown kind, a malformed
// position or specifier operand, a file outside the program, or a position
// past its file or line), before answering any; and 2 when it cannot run: wrong usage, a compiler that
// is missing or not 5.6.3, or a configuration that 5.6.3 rejects.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { loadTypeScript56, parseConfig, programFileName, TS56_VERSION } from './ts56-compiler.mjs';

const KINDS = new Set(['type', 'references', 'specifier']);

// A query this oracle cannot answer as asked; the CLI exits 1 for it.
export class QueryError extends Error {}

// The oracle document for `config` and `queries` ('KIND FILE:LINE:COLUMN').
export function ts56Oracle(configPath, queries, compiler = loadTypeScript56()) {
  const { ts, libraries } = compiler;
  const config = fs.realpathSync(path.resolve(configPath));
  const root = path.dirname(config);
  const service = languageService(ts, parseConfig(ts, config), root);
  try {
    const program = service.getProgram();
    const name = programFileName(root, libraries);
    const at = (sourceFile, offset) => {
      const { line, character } = sourceFile.getLineAndCharacterOfPosition(offset);
      return { line: line + 1, column: character + 1 };
    };
    const located = queries.map(query => ({ query, ...locate(program, root, query) }));
    return {
      config,
      version: TS56_VERSION,
      results: located.map(({ query, kind, sourceFile, offset, target }) => {
        if (kind === 'specifier') return { query, specifiers: specifiers(ts, program, root, sourceFile, target) };
        if (kind === 'type') {
          const info = service.getQuickInfoAtPosition(sourceFile.fileName, offset);
          return {
            query,
            quickInfo: info ? {
              text: ts.displayPartsToString(info.displayParts),
              span: { start: at(sourceFile, info.textSpan.start), end: at(sourceFile, info.textSpan.start + info.textSpan.length) },
            } : null,
          };
        }
        const symbols = service.findReferences(sourceFile.fileName, offset);
        return {
          query,
          references: symbols ? symbols.flatMap(symbol => symbol.references.map(reference => {
            const file = program.getSourceFile(reference.fileName);
            return { file: name(file.fileName), ...at(file, reference.textSpan.start), definition: !!reference.isDefinition, write: !!reference.isWriteAccess };
          })) : null,
        };
      }),
    };
  } finally {
    service.dispose();
  }
}

// The checker's module specifiers for `target`'s module from `importer`
// (`getSpecifierForModuleSymbol`), or null where `target` is no module.
function specifiers(ts, program, root, importer, target) {
  const checker = program.getTypeChecker();
  const symbol = checker.getSymbolAtLocation(target);
  if (!symbol) return null;
  const host = ts.createModuleSpecifierResolutionHost(program, {
    getCurrentDirectory: () => root,
    readFile: ts.sys.readFile,
    useCaseSensitiveFileNames: () => ts.sys.useCaseSensitiveFileNames,
  });
  const mode = program.getDefaultResolutionModeForFile(importer);
  return ts.moduleSpecifiers.getModuleSpecifiers(symbol, checker, program.getCompilerOptions(), importer, host, {
    importModuleSpecifierPreference: 'project-relative',
    importModuleSpecifierEnding: mode === ts.ModuleKind.ESNext ? 'js' : undefined,
  }, {});
}

// The program's source file for `file`, relative to `root` or absolute;
// throws QueryError.
function programFile(program, root, query, file) {
  const absolute = path.resolve(root, file);
  const sourceFile = program.getSourceFile(absolute) ?? (fs.existsSync(absolute) ? program.getSourceFile(fs.realpathSync(absolute)) : undefined);
  if (!sourceFile) throw new QueryError(`${query}: ${file} is not a file of the program`);
  return sourceFile;
}

// A query's kind, program file, and UTF-16 offset, or a specifier query's
// importer and target; throws QueryError.
function locate(program, root, query) {
  const space = query.indexOf(' ');
  const kind = space < 0 ? query : query.slice(0, space);
  if (!KINDS.has(kind)) throw new QueryError(`${query}: the query kind must be type, references, or specifier`);
  if (kind === 'specifier') {
    const operand = space < 0 ? '' : query.slice(space + 1);
    const equals = operand.indexOf('=');
    if (equals <= 0 || equals === operand.length - 1) throw new QueryError(`${query}: expected specifier IMPORTER=TARGET`);
    return {
      kind,
      sourceFile: programFile(program, root, query, operand.slice(0, equals)),
      target: programFile(program, root, query, operand.slice(equals + 1)),
    };
  }
  const match = /^(.+):([1-9][0-9]*):([1-9][0-9]*)$/.exec(query.slice(space + 1));
  if (space < 0 || !match) throw new QueryError(`${query}: expected ${kind} FILE:LINE:COLUMN with a one-based LINE and COLUMN`);
  const [, file, line, column] = match;
  const sourceFile = programFile(program, root, query, file);
  const starts = sourceFile.getLineStarts();
  if (Number(line) > starts.length) throw new QueryError(`${query}: ${file} has ${starts.length} lines`);
  const start = starts[Number(line) - 1];
  const end = sourceFile.getLineEndOfPosition(start);
  const offset = start + Number(column) - 1;
  if (offset > end) throw new QueryError(`${query}: line ${line} of ${file} has ${end - start} UTF-16 code units`);
  return { kind, sourceFile, offset };
}

// A language service over the parsed configuration's program, reading files
// through 5.6.3's own system host.
function languageService(ts, parsed, root) {
  const host = {
    getCompilationSettings: () => parsed.options,
    getScriptFileNames: () => parsed.fileNames,
    getProjectReferences: () => parsed.projectReferences,
    getScriptVersion: () => '0',
    getScriptSnapshot: fileName => {
      const text = ts.sys.readFile(fileName);
      return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
    },
    getCurrentDirectory: () => root,
    getDefaultLibFileName: options => ts.getDefaultLibFilePath(options),
    useCaseSensitiveFileNames: () => ts.sys.useCaseSensitiveFileNames,
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
    getDirectories: ts.sys.getDirectories,
    realpath: ts.sys.realpath,
  };
  return ts.createLanguageService(host, ts.createDocumentRegistry(ts.sys.useCaseSensitiveFileNames, root));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const [config, ...words] = process.argv.slice(2);
  if (!config || words.length === 0 || words.length % 2 !== 0) {
    console.error('usage: node tools/ts56-oracle.mjs TSCONFIG QUERY [QUERY ...], each (type|references) FILE:LINE:COLUMN or specifier IMPORTER=TARGET');
    process.exit(2);
  }
  const queries = [];
  for (let index = 0; index < words.length; index += 2) queries.push(`${words[index]} ${words[index + 1]}`);
  try {
    process.stdout.write(`${JSON.stringify(ts56Oracle(config, queries), null, 2)}\n`);
  } catch (error) {
    console.error(`ts56-oracle: ${error.message}`);
    process.exit(error instanceof QueryError ? 1 : 2);
  }
}
