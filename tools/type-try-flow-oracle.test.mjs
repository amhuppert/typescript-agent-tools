import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, loadSemanticApi, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-try-flow/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'flow.ts'), 'utf8');
const controls = [...source.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => ({
  marker: match[1], offset: match.index + match[0].length,
}));

test('native try/catch/finally read and return dependencies', async () => {
  assert.equal(controls.length, 21);
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const { marker, offset } of controls) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
      });
      const summary = marker === 'try-return' || marker === 'try-catch-return' || marker === 'finally-return' || /^unreachable-(?:call|try)-after-return$/.test(marker) ? '() => number'
        : marker === 'finally-conditional-return' ? (config === 'tsconfig.json' ? '() => 1 | 2 | undefined' : '() => 1 | 2')
        : /^try-(?:throw|never)/.test(marker) ? '() => void'
        : /first-entry|path-refinement/.test(marker) ? 'string | number' : 'number';
      assert.equal(answer.summary, summary, `${config}: ${marker}`);
      assert.equal(answer.errorType, false, `${config}: ${marker}`);
      assert.equal(answer.history, 'baseline');
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('native assertion metadata retains the consumed missing predicate type', async () => {
  const native = await loadSemanticApi();
  for (const config of ['tsconfig.json', 'loose.json']) {
    const configPath = fs.realpathSync(path.join(directory, config));
    const api = new native.API({ tsserverPath: native.binary, cwd: directory });
    let snapshot;
    try {
      snapshot = api.updateSnapshot({ openProject: configPath });
      const project = snapshot.getProject(configPath);
      assert(project);
      const sourceFile = project.program.getSourceFile(path.join(directory, 'flow.ts'));
      let declaration;
      sourceFile.forEachChild(node => {
        if (node.kind === native.SyntaxKind.FunctionDeclaration && node.name?.text === 'badEffect') declaration = node;
      });
      assert(declaration);
      const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(declaration.name));
      assert.equal(type.isErrorType(), false);
      const [signature] = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
      const predicate = project.checker.getTypePredicateOfSignature(signature);
      // Pinned checker.TypePredicateKindAssertsIdentifier; not exported by API.
      assert.equal(predicate.kind, 3);
      assert.equal(predicate.parameterIndex, 0);
      assert.equal(predicate.type.isErrorType(), true);
      assert.equal(project.checker.typeToString(predicate.type), 'Missing');
    } finally {
      try { snapshot?.dispose(); } finally { api.close(); }
    }
  }
});
