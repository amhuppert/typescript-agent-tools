// Fresh native call and signature-return controls, without diagnostic warmup.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, loadSemanticApi, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-calls/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'consumer.ts'), 'utf8');
const functions = fs.readFileSync(path.join(directory, 'functions.ts'), 'utf8');
function at(name) {
  const matches = [...source.matchAll(new RegExp(`\\b${name}\\b`, 'g'))];
  assert.equal(matches.length, 1, name);
  return { file: 'consumer.ts', ...position(lineStarts(source), matches[0].index) };
}
async function observe(config, name, summary, errorType = false) {
  const answer = await typeOracle(path.join(directory, config), { at: at(name), properties: false });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.summary, summary, `${config}: ${name}`);
  assert.equal(answer.errorType, errorType, `${config}: ${name}`);
  assert.deepEqual(answer.prelude, []);
  return answer;
}

test('initializer quick types fetch single nongeneric returns without checking arguments or bodies', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['importedFixed', 'importedInferred', 'importedOptional', 'importedVoidOmitted', 'importedVoidThis'])
      await observe(config, name, 'number');
    await observe(config, 'importedLiteral', '"ready"');
    await observe(config, 'importedPredicate', 'boolean');
  }
});

test('overload failure results remain distinct from genuine any and error-call results', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['importedRequiredMissing', 'importedTooMany', 'importedTooManyMissing', 'importedWrongArgument', 'importedWrongThis', 'importedMissingRequiredType'])
      await observe(config, name, 'number');
    for (const name of ['importedUntyped', 'importedUntypedZero', 'importedUntypedMissing'])
      await observe(config, name, 'any');
    await observe(config, 'importedUnknown', 'any', config === 'tsconfig.json');
    await observe(config, 'importedMissingCallee', 'any', true);
  }
});

test('generic instantiation and overload order have compiler-defined results', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'importedOverloaded', '"text"');
    await observe(config, 'importedOverloadFailure', 'never');
    await observe(config, 'importedGeneric', '"ready"');
    await observe(config, 'importedExplicitGeneric', 'number');
    await observe(config, 'importedBadTypeArguments', 'number');
  }
});

test('mutual return recovery contrasts with bare self calls and established recursive annotation caches', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'importedMutual', 'any');
    await observe(config, 'importedDirectSelf', 'never');
    await observe(config, 'importedRecursiveAnnotation', '() => typeof recursiveAnnotation');
  }
});

test('same-checker demand phases pin quick-type and lazy return checking schedules', async () => {
  for (const [name, before, after, absent] of [
    ['importedOptional', [], [], [2304, 2554]],
    ['importedMissingRequiredType', [], [], [2304, 2554]],
    ['importedTooManyMissing', [], [], [2304, 2554]],
    ['importedCheckedRequiredMissing', [], [2554], [2304]],
    ['importedCheckedTooManyMissing', [], [2554], [2304]],
    ['importedCheckedMissingRequiredType', [], [2554, 2304], []],
    ['importedMutual', [7023], [7023], []],
  ]) {
    const answer = await compilerHookOracle(path.join(directory, 'tsconfig.json'), { operation: 'type', at: at(name) });
    assert.equal(answer.history, 'baseline');
    for (const code of before) assert.ok(answer.beforeSummary.diagnosticCodes.includes(code), `${name}: missing before-summary ${code}`);
    for (const code of after) assert.ok(answer.diagnosticCodes.includes(code), `${name}: missing after-summary ${code}`);
    for (const code of after.filter(code => !before.includes(code)))
      assert.ok(!answer.beforeSummary.diagnosticCodes.includes(code), `${name}: lazy return demanded before summary`);
    for (const code of absent) {
      assert.ok(!answer.beforeSummary.diagnosticCodes.includes(code), `${name}: unexpected before-summary ${code}`);
      assert.ok(!answer.diagnosticCodes.includes(code), `${name}: unexpected after-summary ${code}`);
    }
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
  }
});

test('a parameter annotation error is distinct from its healthy call-result root and from optional absence', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  for (const name of ['missingRequiredTypeResult', 'optionalResult']) {
    const api = new native.API({ tsserverPath: native.binary, cwd: directory });
    let snapshot;
    try {
      const config = fs.realpathSync(path.join(directory, 'tsconfig.json'));
      snapshot = api.updateSnapshot({ openProject: config });
      const project = snapshot.getProject(config);
      assert.ok(project);
      for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()])
        assert.deepEqual(diagnostics, []);
      const file = project.program.getSourceFile(path.join(directory, 'functions.ts'));
      const declaration = file.statements.flatMap(statement => statement.declarationList?.declarations ?? [])
        .find(declaration => declaration.name.text === name);
      assert.ok(declaration, name);
      // Root demand first uses getQuickTypeOfExpression, which never resolves
      // this single nongeneric call. The later resolved-signature/parameter
      // inspections are separate questions, not retrospective dependencies.
      const result = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(declaration.name));
      assert.equal(project.checker.typeToString(result), 'number');
      assert.equal(result.isErrorType(), false);
      const signature = project.checker.getResolvedSignature(declaration.initializer);
      assert.ok(signature);
      assert.equal(signature.getParameters().length, 1);
      const parameter = project.checker.getTypeOfSymbol(signature.getParameters()[0]);
      assert.equal(project.checker.typeToString(parameter), name === 'optionalResult' ? 'any' : 'Missing');
      assert.equal(parameter.isErrorType(), true);
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
  }
});

test('actual return-body call checking differs from the initializer quick path', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['importedCheckedFixed', 'importedCheckedInferred', 'importedCheckedOptional',
      'importedCheckedVoidOmitted', 'importedCheckedVoidThis', 'importedCheckedRequiredMissing',
      'importedCheckedTooManyMissing', 'importedCheckedMissingRequiredType'])
      await observe(config, name, '() => number');
    await observe(config, 'importedCheckedLiteral', '() => "ready"');
    await observe(config, 'importedCheckedUntypedZero', '() => any');
    await observe(config, 'importedCheckedMissingCallee', '() => any');
  }
});

test('each own mutual function lazily consumes its actual return cycle in both option states', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['mutualA', 'mutualB']) {
      const offset = functions.indexOf(`function ${name}`) + 'function '.length;
      assert.ok(offset >= 'function '.length);
      const selected = { file: 'functions.ts', ...position(lineStarts(functions), offset) };
      const answer = await typeOracle(path.join(directory, config), { at: selected, properties: false });
      assert.equal(answer.summary, '() => any');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.prelude, []);
      const hook = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: selected });
      assert.ok(!hook.beforeSummary.diagnosticCodes.includes(7023), `${config}:${name} before summary`);
      assert.equal(hook.diagnosticCodes.includes(7023), config === 'tsconfig.json', `${config}:${name} after summary`);
      assert.deepEqual(hook.beforeSummary.demandFailures, []);
    }
  }
});
