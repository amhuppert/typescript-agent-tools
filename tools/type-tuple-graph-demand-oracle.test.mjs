import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/tuple-graph-demand/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

test('native tuple aliases hide unresolved operands until the argument collection is requested', async () => {
  const native = await loadSemanticApi();
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [marker, summary, children] of [
      ['hidden', 'HiddenTuple', [true]],
      ['generic', 'GenericTuple<number>', [false, true]],
      ['known', 'KnownTuple', [false]],
    ]) {
      const api = new native.API({ tsserverPath: native.binary, cwd: root });
      let snapshot;
      try {
        snapshot = api.updateSnapshot({ openProject: path.join(root, config) });
        const project = snapshot.getProject(path.join(root, config));
        const program = project.program;
        for (const diagnostics of [program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(), program.getSyntacticDiagnostics(), program.getBindDiagnostics()]) {
          assert.deepEqual(diagnostics.map(diagnostic => diagnostic.code), []);
        }
        const file = program.getSourceFile(path.join(root, 'cases.ts'));
        const tag = `/*#${marker}*/`;
        const node = native.getTokenAtPosition(file, source.indexOf(tag) + tag.length);
        const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(node));
        assert.equal(type.isErrorType(), false, `${config}/${marker}/root`);
        assert.equal(project.checker.typeToString(type), summary, `${config}/${marker}/summary`);
        assert.equal(project.checker.isTupleType(type), true);
        assert.deepEqual(project.checker.getTypeArguments(type).map(argument => argument.isErrorType()), children, `${config}/${marker}/arguments`);
      } finally {
        try { snapshot?.dispose(); } finally { api.close(); }
      }
    }
  }
});

test('native oversized alias identities retain healthy tuple headers and demanded error operands', async () => {
  const native = await loadSemanticApi();
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-tuple-graph-'));
  try {
    for (const length of [261639, 261638, 261637, 261636]) {
      const name = 'A'.repeat(length);
      const text = `type ${name} = [MissingGraph, number];\ninterface Array<T> { length: number; [index: number]: T }\ninterface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }\ndeclare const value: ${name};\n`;
      fs.writeFileSync(path.join(directory, 'cases.ts'), text);
      for (const [strict, exactOptionalPropertyTypes] of [[false, false], [true, false], [true, true]]) {
        const config = path.join(directory, 'tsconfig.json');
        fs.writeFileSync(config, JSON.stringify({ compilerOptions: { strict, exactOptionalPropertyTypes, noLib: true }, files: ['cases.ts'] }));
        const api = new native.API({ tsserverPath: native.binary, cwd: directory });
        let snapshot;
        try {
          snapshot = api.updateSnapshot({ openProject: config });
          const project = snapshot.getProject(config);
          const program = project.program;
          for (const diagnostics of [program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(), program.getSyntacticDiagnostics(), program.getBindDiagnostics()]) assert.deepEqual(diagnostics.map(diagnostic => diagnostic.code), []);
          const file = program.getSourceFile(path.join(directory, 'cases.ts'));
          const node = native.getTokenAtPosition(file, text.lastIndexOf('value:'));
          const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(node));
          assert.equal(type.isErrorType(), false);
          const summary = project.checker.typeToString(type);
          // Native typeToString applies its own 320-byte display cutoff;
          // the alias symbol still retains the complete identity spelling.
          assert.equal(summary, `${'A'.repeat(317)}...`);
          assert.equal(type.getAliasSymbol().name, name);
          assert.equal(project.checker.isTupleType(type), true);
          assert.deepEqual(project.checker.getTypeArguments(type).map(argument => argument.isErrorType()), [true, false]);
        } finally {
          try { snapshot?.dispose(); } finally { api.close(); }
        }
      }
    }
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
