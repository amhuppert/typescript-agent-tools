import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/tuple-optionality/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

test('native tuple display removes optional Missing while preserving explicit Undefined and raw argument identity', async () => {
  const native = await loadSemanticApi();
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    const strict = config !== 'loose.json';
    const exact = config === 'exact.json';
    const optional = strict && !exact ? 'string | undefined' : 'string';
    const explicit = strict ? 'string | undefined' : 'string';
    for (const [marker, summary] of [
      ['unnamed', strict && !exact ? '[(string | undefined)?]' : '[string?]'],
      ['named', `[value?: ${optional}]`],
      ['explicitUnnamed', strict ? '[(string | undefined)?]' : '[string?]'],
      ['explicitNamed', `[value?: ${explicit}]`],
      ['required', `[${explicit}]`],
      ['readonly', `readonly [value?: ${optional}]`],
    ]) {
      const api = new native.API({ tsserverPath: native.binary, cwd: root });
      let snapshot;
      try {
        snapshot = api.updateSnapshot({ openProject: path.join(root, config) });
        const project = snapshot.getProject(path.join(root, config));
        const program = project.program;
        for (const diagnostics of [program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(), program.getSyntacticDiagnostics(), program.getBindDiagnostics()]) assert.deepEqual(diagnostics.map(diagnostic => diagnostic.code), []);
        const file = program.getSourceFile(path.join(root, 'cases.ts'));
        const tag = `/*#${marker}*/`;
        const node = native.getTokenAtPosition(file, source.indexOf(tag) + tag.length);
        const checker = project.checker;
        const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
        assert.equal(type.isErrorType(), false, `${config}/${marker}`);
        assert.equal(checker.typeToString(type), summary, `${config}/${marker}`);
        const argument = checker.getTypeArguments(type)[0];
        assert.equal(checker.typeToString(argument), explicit, `${config}/${marker}/raw`);
        assert.equal(argument.isUnionType(), strict, `${config}/${marker}/raw union`);
        assert.equal(type.getTarget().elementFlags[0], marker === 'required' ? 1 : 2);
      } finally {
        try { snapshot?.dispose(); } finally { api.close(); }
      }
    }
  }
});
