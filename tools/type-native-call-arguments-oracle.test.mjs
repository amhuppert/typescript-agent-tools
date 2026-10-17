// Fresh native contextual scalar argument and relation controls.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, loadSemanticApi, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-call-arguments/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'consumer.ts'), 'utf8');
function at(name) {
  const offset = source.indexOf(`${name},`);
  assert.ok(offset >= 0, name);
  return { file: 'consumer.ts', ...position(lineStarts(source), offset) };
}

test('fresh scalar contextual calls retain native return types in both null profiles', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary] of [
      ['importedNumber', '() => number'], ['importedString', '() => string'],
      ['importedLiteral', '() => "ready"'], ['importedBoolean', '() => boolean'],
      ['importedUnionNumber', '() => number'], ['importedUnionString', '() => number'],
      ['importedAny', '() => number'], ['importedUnknown', '() => number'],
      ['importedOptional', '() => number'], ['importedVoid', '() => number'],
      ['importedWrongPrimitive', '() => number'], ['importedWrongLiteral', '() => "ready"'],
      ['importedNever', '() => number'], ['importedNull', '() => number'],
      ['importedThisBeforeMissing', '() => number'], ['importedWrongBeforeMissing', '() => number'],
      ['importedMissing', '() => number'], ['importedAnyMissing', '() => any'],
      ['importedUnionSource', '() => number'], ['importedUnionRejected', '() => string'],
      ['importedLiteralUnion', '() => string'],
      ['importedNumericEquivalent', '() => 100'], ['importedNumericRejected', '() => 100'],
      ['importedBigint', '() => bigint'], ['importedBigintEquivalent', '() => 10n'],
      ['importedBigintRejected', '() => 10n'], ['importedFalse', '() => false'],
      ['importedBooleanRejected', '() => false'], ['importedSymbol', '() => symbol'],
      ['importedEscaped', '() => "ready"'], ['importedOptionalUndefined', '() => number'],
    ]) {
      const answer = await typeOracle(path.join(directory, config), { at: at(name), properties: false });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'value');
      assert.equal(answer.summary, summary, `${config}: ${name}`);
      assert.equal(answer.errorType, false, `${config}: ${name}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('same-checker applicability observes first-failure order without treating every diagnostic as invalid', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, required, absent] of [
      ['importedWrongLiteral', [2345], [2304, 2684]],
      ['importedWrongPrimitive', [2345], [2304, 2684]],
      ['importedNever', [2345], [2304, 2684]],
      ['importedNumericRejected', [2345], [2304, 2684]],
      ['importedBigintRejected', [2345], [2304, 2684]],
      ['importedBooleanRejected', [2345], [2304, 2684]],
      ['importedThisBeforeMissing', [2684], [2304, 2345]],
      ['importedWrongBeforeMissing', [2345], [2304, 2684]],
      ['importedMissing', [2304], [2345, 2684]],
      ['importedAnyMissing', [2304], [2345, 2684]],
      ['importedUnionRejected', [2345], [2304, 2684]],
      ['importedNull', config === 'tsconfig.json' ? [2345] : [], [2304, 2684, ...(config === 'loose.json' ? [2345] : [])]],
    ]) {
      const answer = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: at(name) });
      assert.equal(answer.history, 'baseline');
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      for (const code of required) {
        assert.ok(!answer.beforeSummary.diagnosticCodes.includes(code), `${config}:${name}: lazy return checked early`);
        assert.ok(answer.diagnosticCodes.includes(code), `${config}:${name}: missing ${code}`);
      }
      for (const code of absent)
        assert.ok(!answer.diagnosticCodes.includes(code), `${config}:${name}: unexpected ${code}`);
    }
  }
});

test('fresh native scalar assignability is directional across literals, intrinsic types and primitive unions', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  for (const configName of ['tsconfig.json', 'loose.json']) {
    const strict = configName === 'tsconfig.json';
    for (const [sourceName, targetName, expected] of [
      ['ready', 'text', true], ['text', 'ready', false], ['ready', 'other', false],
      ['one', 'number', true], ['one', 'two', false], ['ten', 'bigint', true],
      ['yes', 'boolean', true], ['boolean', 'yes', false], ['yes', 'no', false],
      ['symbol', 'symbol', true], ['any', 'text', true], ['unknown', 'text', false],
      ['never', 'number', true], ['text', 'unknown', true], ['number', 'any', true],
      ['any', 'never', false], ['never', 'never', true],
      ['undefinedValue', 'voidValue', true], ['voidValue', 'undefinedValue', false],
      ['nullable', 'number', !strict], ['undefinedValue', 'text', !strict],
      ['mixed', 'text', false], ['mixed', 'mixed', true], ['literalUnion', 'text', true],
      ['literalUnion', 'ready', false], ['text', 'optional', true],
    ]) {
      const api = new native.API({ tsserverPath: native.binary, cwd: directory });
      let snapshot;
      try {
        const config = fs.realpathSync(path.join(directory, configName));
        snapshot = api.updateSnapshot({ openProject: config });
        const project = snapshot.getProject(config);
        assert.ok(project);
        for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()])
          assert.deepEqual(diagnostics, []);
        const file = project.program.getSourceFile(path.join(directory, 'relations.ts'));
        const declarations = file.statements.flatMap(statement => statement.declarationList?.declarations ?? []);
        const types = [sourceName, targetName].map(name => {
          const declaration = declarations.find(declaration => declaration.name.text === name);
          assert.ok(declaration, name);
          return project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(declaration.name));
        });
        assert.equal(project.checker.isTypeAssignableTo(types[0], types[1]), expected,
          `${configName}: ${sourceName} -> ${targetName}`);
      } finally { try { snapshot?.dispose(); } finally { api.close(); } }
    }
  }
});
