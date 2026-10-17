import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi, lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle } from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-signatures/', import.meta.url));
async function withChecker(config, action) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    const configPath = fs.realpathSync(path.join(root, config));
    snapshot = api.updateSnapshot({ openProject: configPath });
    const project = snapshot.getProject(configPath);
    assert.ok(project);
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()])
      assert.deepEqual(diagnostics, []);
    const source = project.program.getSourceFile(path.join(root, 'functions.ts'));
    assert.ok(source);
    const symbol = name => {
      const node = source.statements.find(node => node.name?.text === name);
      assert.ok(node, name);
      const result = project.checker.getSymbolAtLocation(node.name);
      assert.ok(result, name);
      return result;
    };
    const describe = type => {
      assert.ok(type);
      return { summary: project.checker.typeToString(type), error: type.isErrorType() };
    };
    const signature = sig => ({
      thisParameter: sig.getThisParameter() && { name: sig.getThisParameter().name, ...describe(project.checker.getTypeOfSymbol(sig.getThisParameter())) },
      parameters: sig.getParameters().map(parameter => ({ name: parameter.name, ...describe(project.checker.getTypeOfSymbol(parameter)) })),
      returnType: describe(project.checker.getReturnTypeOfSignature(sig)),
      construct: sig.isConstruct,
      rest: sig.hasRestParameter,
      typeParameters: sig.getTypeParameters().map(describe),
      declaration: sig.declaration.resolve(project),
    });
    return action({ native, project, source, symbol, describe, signature });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('generic signature identities are canonical while const and escaped parameter display follow the native serializer', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary, decoded, printed, constraint, defaultSummary] of [
      ['generic', '<T extends string = "ready">(value: T) => T', 'T', 'T', 'string', '"ready"'],
      ['plainGeneric', '<T>(value: T) => T', 'T', 'T', undefined, undefined],
      ['constGeneric', '<const T extends string = "ready">(value: T) => T', 'T', 'T', 'string', '"ready"'],
      ['escapedGeneric', '<\\u0055 extends number = 1>(value: \\u0055) => \\u0055', 'U', '\\u0055', 'number', '1'],
      ['genericAny', '<T extends unknown = any>(value: T) => T', 'T', 'T', 'unknown', 'any'],
      ['varianceIn', '<in T>(value: T) => T', 'T', 'T', undefined, undefined],
      ['varianceOut', '<out T>(value: T) => T', 'T', 'T', undefined, undefined],
      ['varianceBoth', '<in out T>(value: T) => T', 'T', 'T', undefined, undefined],
      ['constVariance', '<const in out T>(value: T) => T', 'T', 'T', undefined, undefined],
    ]) {
      await withChecker(config, ({ native, project, source, symbol, describe }) => {
        const type = project.checker.getTypeOfSymbol(symbol(name));
        assert.deepEqual(describe(type), { summary, error: false });
        const calls = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
        assert.equal(calls.length, 1);
        const parameters = calls[0].getTypeParameters();
        assert.equal(parameters.length, 1);
        const parameter = parameters[0];
        assert.equal(parameter.getSymbol().name, decoded);
        assert.deepEqual(describe(parameter), { summary: printed, error: false });
        assert.equal(project.checker.getTypeOfSymbol(calls[0].getParameters()[0]).id, parameter.id);
        assert.equal(project.checker.getReturnTypeOfSignature(calls[0]).id, parameter.id);
        const declaration = calls[0].declaration.resolve(project);
        const own = declaration.typeParameters[0];
        assert.equal(project.checker.getDeclaredTypeOfSymbol(project.checker.getSymbolAtLocation(own.name)).id, parameter.id);
        assert.equal(project.checker.getConstraintOfTypeParameter(parameter)?.isErrorType(), constraint === undefined ? undefined : false);
        assert.equal(project.checker.getConstraintOfTypeParameter(parameter) && project.checker.typeToString(project.checker.getConstraintOfTypeParameter(parameter)), constraint);
        assert.equal(own.defaultType && project.checker.typeToString(project.checker.getTypeAtLocation(own.defaultType)), defaultSummary);
        assert.equal(source.text.slice(own.name.getStart(source), own.name.end), printed);
      });
    }
  }
});

test('generic type syntax and named interfaces acquire identities without forcing their missing constraints', async () => {
  for (const [name, kind, summary] of [
    ['GenericCall', 'Call', 'GenericCall'],
    ['GenericConstructor', 'Construct', 'GenericConstructor'],
    ['GenericCallable', 'Call', 'GenericCallable'],
    ['GenericMissing', 'Call', 'GenericMissing'],
  ]) {
    await withChecker('tsconfig.json', ({ native, project, symbol, describe }) => {
      const type = project.checker.getDeclaredTypeOfSymbol(symbol(name));
      assert.deepEqual(describe(type), { summary, error: false });
      const signatures = project.checker.getSignaturesOfType(type, native.SignatureKind[kind]);
      assert.equal(signatures.length, 1);
      const signature = signatures[0];
      const parameters = signature.getTypeParameters();
      assert.equal(parameters.length, 1);
      assert.deepEqual(describe(parameters[0]), { summary: 'T', error: false });
      assert.equal(project.checker.getTypeOfSymbol(signature.getParameters()[0]).id, parameters[0].id);
      if (name !== 'GenericConstructor') assert.equal(project.checker.getReturnTypeOfSignature(signature).id, parameters[0].id);
      else {
        const returned = project.checker.getReturnTypeOfSignature(signature);
        assert.deepEqual(describe(returned), { summary: '{ value: T; }', error: false });
        assert.equal(project.checker.getTypeOfSymbol(project.checker.getPropertiesOfType(returned)[0]).id, parameters[0].id);
      }
      if (name === 'GenericMissing') {
        assert.equal(project.checker.getConstraintOfTypeParameter(parameters[0]).isErrorType(), true);
        const declaration = signature.declaration.resolve(project);
        assert.equal(project.checker.getTypeAtLocation(declaration.typeParameters[0].defaultType).isErrorType(), true);
      }
    });
  }
});

test('generic overloads retain distinct parameter symbols and declaration order across renamed imports', async () => {
  await withChecker('tsconfig.json', ({ native, project, source, symbol, describe }) => {
    const type = project.checker.getTypeOfSymbol(symbol('genericOverloaded'));
    assert.deepEqual(describe(type), { summary: '{ <T extends string>(value: T): T; <U extends number>(value: U): U; }', error: false });
    const calls = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
    assert.equal(calls.length, 2);
    const identities = calls.map(signature => signature.getTypeParameters()[0]);
    assert.notEqual(identities[0].id, identities[1].id);
    assert.deepEqual(identities.map(parameter => parameter.getSymbol().name), ['T', 'U']);
    for (const [index, signature] of calls.entries()) {
      assert.equal(project.checker.getTypeOfSymbol(signature.getParameters()[0]).id, identities[index].id);
      assert.equal(project.checker.getReturnTypeOfSignature(signature).id, identities[index].id);
      assert.equal(signature.declaration.resolve(project).body, undefined);
      assert.equal(position(lineStarts(source.text), signature.declaration.resolve(project).name.getStart(source)).line, 43 + index);
    }
  });
  const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
  const answer = await typeOracle(path.join(root, 'tsconfig.json'), { at: { file: 'consumer.ts', ...position(lineStarts(source), source.indexOf('importedGenericOverloaded')) } });
  assert.equal(answer.kind, 'value');
  assert.equal(answer.summary, '{ <T extends string>(value: T): T; <U extends number>(value: U): U; }');
  assert.equal(answer.errorType, false);
  assert.deepEqual(answer.prelude, []);
  await withChecker('tsconfig.json', ({ native, project, symbol, describe }) => {
    const type = project.checker.getTypeOfSymbol(symbol('sameGenericName'));
    assert.deepEqual(describe(type), { summary: '{ <T extends string>(value: T): T; <T extends number>(value: T): T; }', error: false });
    const calls = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
    assert.equal(calls.length, 2);
    const parameters = calls.map(signature => signature.getTypeParameters()[0]);
    assert.deepEqual(parameters.map(parameter => parameter.getSymbol().name), ['T', 'T']);
    assert.notEqual(parameters[0].id, parameters[1].id, 'spelling equality is not canonical parameter identity');
    assert.deepEqual(parameters.map(parameter => describe(project.checker.getConstraintOfTypeParameter(parameter)).summary), ['string', 'number']);
    for (const [index, signature] of calls.entries()) {
      assert.equal(project.checker.getTypeOfSymbol(signature.getParameters()[0]).id, parameters[index].id);
      assert.equal(project.checker.getReturnTypeOfSignature(signature).id, parameters[index].id);
    }
  });
});

test('annotated function signatures preserve optionality questions and avoid their invalid bodies', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const strict = config === 'tsconfig.json';
    await withChecker(config, ({ native, project, source, symbol, describe, signature }) => {
      const type = project.checker.getTypeOfSymbol(symbol('annotated'));
      assert.equal(type.isErrorType(), false);
      const signatures = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
      assert.equal(signatures.length, 1);
      const sig = signature(signatures[0]);
      assert.deepEqual(sig.thisParameter, { name: 'this', summary: '{ label: string; }', error: false });
      assert.deepEqual(sig.parameters, [
        { name: 'required', summary: 'string', error: false },
        { name: 'optional', summary: strict ? 'number | undefined' : 'number', error: false },
        { name: 'initialized', summary: 'boolean', error: false },
      ]);
      assert.deepEqual(sig.returnType, { summary: 'number', error: false });
      assert.equal(sig.construct, false); assert.equal(sig.rest, false);
      assert.deepEqual(sig.typeParameters, []);
      assert.equal(sig.declaration.kind, native.SyntaxKind.FunctionDeclaration);
      assert.ok(sig.declaration.body, 'body is present but not a return-type dependency');
      assert.deepEqual(sig.declaration.parameters.map(parameter => [parameter.name.text, !!parameter.questionToken, !!parameter.initializer, !!parameter.dotDotDotToken]), [
        ['this', false, false, false], ['required', false, false, false], ['optional', true, false, false], ['initialized', false, true, false],
      ]);
      assert.equal(describe(type).summary, `(this: { label: string; }, required: string, optional?: ${strict ? 'number | undefined' : 'number'}, initialized?: boolean) => number`);
      const bodyless = project.checker.getTypeOfSymbol(symbol('bodyless'));
      const bodylessSig = signature(project.checker.getSignaturesOfType(bodyless, native.SignatureKind.Call)[0]);
      assert.deepEqual(bodylessSig.returnType, { summary: 'any', error: false });
      assert.equal(describe(bodyless).summary, '(value: string) => any');
    });
  }
});

test('function and constructor type aliases and interface collections retain native signature ordering', async () => {
  await withChecker('tsconfig.json', ({ native, project, source, symbol, describe, signature }) => {
    const declared = name => project.checker.getDeclaredTypeOfSymbol(symbol(name));
    const call = declared('CallType');
    assert.deepEqual(describe(call), { summary: 'CallType', error: false });
    assert.equal(call.getAliasSymbol().id, symbol('CallType').id);
    assert.deepEqual(call.getAliasSymbol().declarations.map(handle => {
      const node = handle.resolve(project);
      return [position(lineStarts(source.text), node.name.getStart(source)), position(lineStarts(source.text), node.name.end)];
    }), [[{ line: 5, column: 13 }, { line: 5, column: 21 }]]);
    const callSig = signature(project.checker.getSignaturesOfType(call, native.SignatureKind.Call)[0]);
    assert.deepEqual(callSig.parameters.map(parameter => [parameter.name, parameter.summary]), [['required', 'string'], ['optional', 'number | undefined']]);
    assert.deepEqual(callSig.returnType, { summary: 'boolean', error: false });
    assert.deepEqual(callSig.thisParameter, { name: 'this', summary: '{ label: string; }', error: false });
    assert.equal(callSig.declaration.kind, native.SyntaxKind.FunctionType);
    const construct = declared('ConstructorType');
    assert.deepEqual(describe(construct), { summary: 'ConstructorType', error: false });
    assert.equal(construct.getAliasSymbol().id, symbol('ConstructorType').id);
    assert.deepEqual(construct.getAliasSymbol().declarations.map(handle => {
      const node = handle.resolve(project);
      return [position(lineStarts(source.text), node.name.getStart(source)), position(lineStarts(source.text), node.name.end)];
    }), [[{ line: 6, column: 13 }, { line: 6, column: 28 }]]);
    const constructSig = signature(project.checker.getSignaturesOfType(construct, native.SignatureKind.Construct)[0]);
    assert.equal(constructSig.construct, true);
    assert.deepEqual(constructSig.returnType, { summary: '{ value: number; }', error: false });
    const callable = declared('Callable');
    assert.deepEqual(describe(callable), { summary: 'Callable', error: false });
    assert.equal(callable.getAliasSymbol(), undefined);
    assert.deepEqual(project.checker.getPropertiesOfType(callable), []);
    const calls = project.checker.getSignaturesOfType(callable, native.SignatureKind.Call).map(signature);
    const constructs = project.checker.getSignaturesOfType(callable, native.SignatureKind.Construct).map(signature);
    assert.equal(calls.length, 1); assert.equal(constructs.length, 1);
    assert.equal(calls[0].declaration.kind, native.SyntaxKind.CallSignature);
    assert.equal(constructs[0].declaration.kind, native.SyntaxKind.ConstructSignature);
  });
});

test('renamed cross-file function questions keep signatures rather than import-name labels', async () => {
  const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
  for (const [name, summary] of [
    ['importedDeclared', '(required: string, optional?: number | undefined) => boolean'],
    ['importedBodyless', '(value: string) => any'],
  ]) {
    const answer = await typeOracle(path.join(root, 'tsconfig.json'), { at: { file: 'consumer.ts', ...position(lineStarts(source), source.indexOf(name)) } });
    assert.equal(answer.kind, 'value');
    assert.equal(answer.summary, summary);
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.prelude, []);
  }
});

test('native signature origins and consumed unresolved parameter recovery are exact', async () => {
  await withChecker('tsconfig.json', ({ native, project, source, symbol, signature }) => {
    const nameSpan = node => [position(lineStarts(source.text), node.getStart(source)), position(lineStarts(source.text), node.end)];
    const functionType = project.checker.getTypeOfSymbol(symbol('annotated'));
    const functionSig = signature(project.checker.getSignaturesOfType(functionType, native.SignatureKind.Call)[0]);
    assert.deepEqual(nameSpan(functionSig.declaration.name), [{ line: 1, column: 17 }, { line: 1, column: 26 }]);
    const call = project.checker.getDeclaredTypeOfSymbol(symbol('CallType'));
    const callSig = signature(project.checker.getSignaturesOfType(call, native.SignatureKind.Call)[0]);
    assert.deepEqual(nameSpan(callSig.declaration), [{ line: 5, column: 24 }, { line: 5, column: 97 }]);
    const invalid = project.checker.getTypeOfSymbol(symbol('invalid'));
    assert.equal(invalid.isErrorType(), false, 'anonymous function root itself is not errorType');
    const invalidSig = signature(project.checker.getSignaturesOfType(invalid, native.SignatureKind.Call)[0]);
    assert.equal(invalidSig.parameters[0].error, true, 'the requested missing annotation consumes recovery');
    assert.equal(invalidSig.returnType.error, false);
  });
});

test('canonical generic parameters, explicit predicates and adjacent overloads expose distinct native questions', async () => {
  await withChecker('tsconfig.json', ({ native, project, source, symbol, describe }) => {
    const calls = name => project.checker.getSignaturesOfType(project.checker.getTypeOfSymbol(symbol(name)), native.SignatureKind.Call);
    const generic = calls('generic')[0];
    const parameters = generic.getTypeParameters(); assert.equal(parameters.length, 1);
    const parameter = parameters[0];
    assert.equal(describe(parameter).summary, 'T');
    assert.equal(project.checker.getTypeOfSymbol(generic.getParameters()[0]).id, parameter.id);
    assert.equal(project.checker.getReturnTypeOfSignature(generic).id, parameter.id);
    assert.deepEqual(describe(project.checker.getConstraintOfTypeParameter(parameter)), { summary: 'string', error: false });
    const genericDeclaration = generic.declaration.resolve(project);
    assert.deepEqual(describe(project.checker.getTypeAtLocation(genericDeclaration.typeParameters[0].defaultType)), { summary: '\"ready\"', error: false });
    assert.equal(project.checker.getDeclaredTypeOfSymbol(project.checker.getSymbolAtLocation(genericDeclaration.typeParameters[0].name)).id, parameter.id);
    for (const [name, kind, summary, asserted] of [['isText', 1, 'boolean', 'string'], ['assertText', 3, 'void', 'string'], ['assertPresent', 3, 'void', undefined]]) {
      const signature = calls(name)[0];
      const predicate = project.checker.getTypePredicateOfSignature(signature);
      assert.ok(predicate, name); assert.equal(predicate.kind, kind);
      assert.equal(predicate.parameterName, 'value'); assert.equal(predicate.parameterIndex, 0);
      assert.equal(predicate.type && describe(predicate.type).summary, asserted);
      assert.deepEqual(describe(project.checker.getReturnTypeOfSignature(signature)), { summary, error: false });
    }
    const overloads = calls('overloaded'); assert.equal(overloads.length, 2);
    assert.deepEqual(overloads.map(signature => describe(project.checker.getReturnTypeOfSignature(signature)).summary), ['string', 'number']);
    assert.deepEqual(overloads.map(signature => position(lineStarts(source.text), signature.declaration.resolve(project).name.getStart(source))), [{ line: 12, column: 17 }, { line: 13, column: 17 }]);
    assert.ok(overloads.every(signature => !signature.declaration.resolve(project).body));
    assert.equal(calls('annotated').length, 1, 'an ordinary function implementation remains a callable signature');
  });
});

test('defaulted parameters before required parameters retain native strong arity without forcing initializer or body checks', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await withChecker(config, ({ native, project, symbol, describe, signature }) => {
      const strict = config === 'tsconfig.json';
      for (const [name, expected, parameters] of [
        ['defaultsBeforeRequired', `(first: ${strict ? 'number | undefined' : 'number'}, required: string, last?: boolean) => number`, [['first', 'number'], ['required', 'string'], ['last', 'boolean']]],
        ['defaultBeforeVoid', `(first: ${strict ? 'number | undefined' : 'number'}, required: void) => number`, [['first', 'number'], ['required', 'void']]],
        ['thisBeforeDefaults', `(this: string, first: ${strict ? 'number | undefined' : 'number'}, required: string, last?: boolean) => number`, [['first', 'number'], ['required', 'string'], ['last', 'boolean']]],
        ['questionBeforeRequired', `(first?: ${strict ? 'number | undefined' : 'number'}, required: string) => number`, [['first', strict ? 'number | undefined' : 'number'], ['required', 'string']]],
        ['explicitUndefined', `(first: ${strict ? 'number | undefined' : 'number'}, required: string) => number`, [['first', strict ? 'number | undefined' : 'number'], ['required', 'string']]],
        ['inferredDefaultsBeforeRequired', `(first: ${strict ? 'number | undefined' : 'number'}, required: string) => number`, [['first', 'number'], ['required', 'string']]],
      ]) {
        const type = project.checker.getTypeOfSymbol(symbol(name));
        assert.deepEqual(describe(type), { summary: expected, error: false });
        const call = signature(project.checker.getSignaturesOfType(type, native.SignatureKind.Call)[0]);
        assert.deepEqual(call.parameters.map(parameter => [parameter.name, parameter.summary]), parameters);
        assert.deepEqual(call.returnType, { summary: 'number', error: false });
      }
    });
  }
});

test('literal-alias parameter and return serialization follows native type identity rather than source spelling', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await withChecker(config, ({ native, project, symbol, describe, signature }) => {
      const word = project.checker.getDeclaredTypeOfSymbol(symbol('Word'));
      assert.deepEqual(describe(word), { summary: '"ready"', error: false });
      assert.equal(word.getAliasSymbol(), undefined);
      const type = project.checker.getTypeOfSymbol(symbol('literalAlias'));
      assert.deepEqual(describe(type), { summary: '(value: "ready") => "ready"', error: false });
      const call = project.checker.getSignaturesOfType(type, native.SignatureKind.Call)[0];
      assert.equal(project.checker.getTypeOfSymbol(call.getParameters()[0]).id, word.id);
      assert.equal(project.checker.getReturnTypeOfSignature(call).id, word.id);
      assert.deepEqual(signature(call).parameters, [{ name: 'value', summary: '"ready"', error: false }]);
    });
  }
});

test('only a same-parent same-kind adjacent overload implementation is excluded from ordered declarations', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await withChecker(config, ({ native, project, source, symbol, describe }) => {
      for (const [name, summaries, rootSummary] of [
        ['overloaded', ['string', 'number'], '{ (value: string): string; (value: number): number; }'],
        ['separated', ['string', 'number'], '{ (value: string): string; (value: number): number; }'],
        ['commented', ['string'], '(value: string) => string'],
      ]) {
        const type = project.checker.getTypeOfSymbol(symbol(name));
        assert.deepEqual(describe(type), { summary: rootSummary, error: false });
        const calls = project.checker.getSignaturesOfType(type, native.SignatureKind.Call);
        assert.deepEqual(calls.map(signature => describe(project.checker.getReturnTypeOfSignature(signature)).summary), summaries);
        const declarations = symbol(name).declarations.map(handle => handle.resolve(project));
        assert.ok(declarations.every(node => node.kind === native.SyntaxKind.FunctionDeclaration));
        const last = declarations.at(-1), previous = declarations.at(-2);
        assert.ok(last.body);
        if (name === 'separated') assert.notEqual(last.pos, previous.end);
        else assert.equal(last.pos, previous.end);
        assert.deepEqual(calls.map(signature => signature.declaration.resolve(project).name.getStart(source)), declarations.slice(0, name === 'separated' ? undefined : -1).map(node => node.name.getStart(source)));
      }
      const namespace = source.statements.find(node => node.name?.text === 'Unrelated');
      const declaration = namespace.body.statements[0];
      const target = project.checker.getSymbolAtLocation(declaration.name);
      assert.notEqual(target.id, symbol('overloaded').id, 'same spelling in another canonical scope is a different symbol');
      assert.deepEqual(describe(project.checker.getTypeOfSymbol(target)), { summary: '(value: boolean) => boolean', error: false });
      const joined = source.statements.filter(node => node.name?.text === 'Joined').map(node => node.body.statements[0]);
      assert.equal(joined.length, 2);
      const joinedSymbol = project.checker.getSymbolAtLocation(joined[0].name);
      assert.equal(project.checker.getSymbolAtLocation(joined[1].name).id, joinedSymbol.id);
      assert.notEqual(joined[0].parent, joined[1].parent);
      const joinedType = project.checker.getTypeOfSymbol(joinedSymbol);
      assert.deepEqual(describe(joinedType), { summary: '{ (value: string): string; (value: number): number; }', error: false });
      assert.equal(project.checker.getSignaturesOfType(joinedType, native.SignatureKind.Call).length, 2);
    });
  }
});

test('explicit predicate subjects and asserted child failures preserve their native question distinctions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await withChecker(config, ({ native, project, symbol, describe }) => {
      for (const [name, kind, index, asserted, rootSummary, returnSummary] of [
        ['isText', 1, 0, 'string', '(value: unknown) => value is string', 'boolean'],
        ['assertText', 3, 0, 'string', '(value: unknown) => asserts value is string', 'void'],
        ['assertPresent', 3, 0, undefined, '(value: unknown) => asserts value', 'void'],
        ['isThis', 0, 0, 'string', '(this: unknown) => this is string', 'boolean'],
        ['assertThis', 2, 0, 'string', '(this: unknown) => asserts this is string', 'void'],
      ]) {
        const type = project.checker.getTypeOfSymbol(symbol(name));
        assert.deepEqual(describe(type), { summary: rootSummary, error: false });
        const signature = project.checker.getSignaturesOfType(type, native.SignatureKind.Call)[0];
        const predicate = project.checker.getTypePredicateOfSignature(signature);
        assert.equal(predicate.kind, kind); assert.equal(predicate.parameterIndex, index);
        assert.equal(predicate.type && describe(predicate.type).summary, asserted);
        assert.deepEqual(describe(project.checker.getReturnTypeOfSignature(signature)), { summary: returnSummary, error: false });
      }
      const type = project.checker.getTypeOfSymbol(symbol('predicateMissing'));
      assert.equal(type.isErrorType(), false);
      const signature = project.checker.getSignaturesOfType(type, native.SignatureKind.Call)[0];
      assert.deepEqual(describe(project.checker.getReturnTypeOfSignature(signature)), { summary: 'boolean', error: false });
      assert.equal(project.checker.getTypePredicateOfSignature(signature).type.isErrorType(), true);
      const named = project.checker.getDeclaredTypeOfSymbol(symbol('PredicateContainer'));
      assert.deepEqual(describe(named), { summary: 'PredicateContainer', error: false });
      const namedCall = project.checker.getSignaturesOfType(named, native.SignatureKind.Call)[0];
      assert.deepEqual(describe(project.checker.getReturnTypeOfSignature(namedCall)), { summary: 'boolean', error: false });
      assert.equal(project.checker.getTypePredicateOfSignature(namedCall).type.isErrorType(), true);
    });
  }
});
