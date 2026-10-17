import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-type-order/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = [
  ['names', 'Alpha | Zebra', ['Alpha', 'Zebra']],
  ['arguments', 'Box<"a"> | Box<"z">', ['Box<"a">', 'Box<"z">']],
  ['intersectionLists', '(Alpha & Zebra) | (Zebra & Alpha)', ['Alpha & Zebra', 'Zebra & Alpha']],
  ['intersectionLengths', '(Gamma & Alpha) | (Zebra & Alpha & Gamma)', ['Gamma & Alpha', 'Zebra & Alpha & Gamma']],
  ['aliasNames', 'AlphaAlias | ZebraAlias', ['AlphaAlias', 'ZebraAlias']],
  ['aliasArguments', 'Choice<"a"> | Choice<"z">', ['"a"', '"z"', '{ choice: "a"; }', '{ choice: "z"; }']],
  ['anonymous', '{ early: string; } | { late: number; }', ['{ early: string; }', '{ late: number; }']],
  ['functions', '(() => string) | (() => number)', ['() => string', '() => number']],
  ['numbers', '-2 | 3 | 10', ['-2', '3', '10']],
  ['strings', '"A" | "a" | "z"', ['"A"', '"a"', '"z"']],
  ['booleans', 'boolean', ['false', 'true']],
];

async function check(config, warm) {
  const native = await loadSemanticApi();
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: path.join(root, config)});
    const project = snapshot.getProject(path.join(root, config));
    assert.deepEqual([
      project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics(),
    ].map(rows => rows.map(row => row.code)), [[], [], [], []]);
    const file = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const checker = project.checker;
    const get = marker => {
      const tag = `/*#${marker}*/`;
      const start = source.indexOf(tag);
      assert.notEqual(start, -1, marker);
      assert.equal(source.indexOf(tag, start + tag.length), -1, marker);
      const node = native.getTokenAtPosition(file, start + tag.length);
      return checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
    };
    for (const marker of warm) assert.equal(get(marker).isErrorType(), false);
    const rows = [...expected];
    const files = config === 'tsconfig.json'
      ? ['{ first: string; }', '{ second: number; }']
      : ['{ second: number; }', '{ first: string; }'];
    rows.push(['fileOrder', files.join(' | '), files]);
    for (const [marker, summary, alternatives] of rows) {
      const type = get(marker);
      assert.equal(type.isErrorType(), false, marker);
      assert.equal(checker.typeToString(type), summary, `${config}/${marker}/${warm}`);
      assert.deepEqual(type.getTypes().map(child => checker.typeToString(child)), alternatives, marker);
      assert.equal(get(marker).id, type.id, `repeat identity ${marker}`);
    }
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

for (const config of ['tsconfig.json', 'reversed.json']) {
  test(`native CompareTypes uses graph facts across acquisition histories (${config})`, async () => {
    for (const warm of [[], ['late', 'early', 'lateFunction', 'earlyFunction'], ['earlyFunction', 'lateFunction', 'early', 'late']]) {
      await check(config, warm);
    }
  });
}
