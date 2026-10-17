import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-anonymous-types/', import.meta.url));
const configs = ['tsconfig.json', 'loose.json'];

async function fresh(config, operation, filename = 'cases.ts') {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: directory });
  let snapshot;
  try {
    const configPath = path.join(directory, config);
    snapshot = api.updateSnapshot({ openProject: configPath });
    const project = snapshot.getProject(configPath), checker = project.checker;
    const file = project.program.getSourceFile(path.join(directory, filename));
    return operation({ native, project, checker, file });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

function declaration(file, name) {
  const found = file.statements.flatMap(statement => statement.declarationList?.declarations ?? [])
    .find(declaration => declaration.name.text === name);
  assert.ok(found, name);
  return found;
}

test('native function and constructor annotations have a TypeLiteral node symbol and a distinct reserved Signature member', async () => {
  for (const config of configs) for (const name of ['func', 'ctor', 'generic', 'abstractCtor']) {
    await fresh(config, ({ native, project, checker, file }) => {
      const own = declaration(file, name), typeNode = own.type;
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(own.name));
      const symbol = type.getSymbol();
      assert.equal(type.flags, native.TypeFlags.Object);
      assert.equal(type.isErrorType(), false);
      assert.ok(symbol, 'the type has its native anonymous symbol');
      assert.equal(symbol.flags, native.SymbolFlags.TypeLiteral);
      assert.equal(symbol.declarations.length, 1);
      assert.equal(symbol.declarations[0].resolve(project), typeNode);
      // GetSymbolAtLocation is a different public operation from the actual
      // binder node.Symbol; nil here cannot prove that no node symbol exists.
      assert.equal(checker.getSymbolAtLocation(typeNode), undefined);
      const members = [...symbol.getMembers().values()];
      assert.equal(members.length, 1);
      const [signatureSymbol] = members;
      assert.equal(signatureSymbol.flags, native.SymbolFlags.Signature);
      assert.notEqual(signatureSymbol.id, symbol.id);
      assert.equal(signatureSymbol.getParent(), undefined);
      assert.equal(signatureSymbol.declarations.length, 1);
      assert.equal(signatureSymbol.declarations[0].resolve(project), typeNode);
      const constructor = native.SyntaxKind[typeNode.kind] === 'ConstructorType';
      const signatures = checker.getSignaturesOfType(type, constructor ? native.SignatureKind.Construct : native.SignatureKind.Call);
      assert.equal(signatures.length, 1);
      assert.equal(signatures[0].declaration.resolve(project), typeNode);
      assert.equal(checker.getSignaturesOfType(type, constructor ? native.SignatureKind.Call : native.SignatureKind.Construct).length, 0);
    });
  }
});

test('native anonymous function and constructor summaries preserve generic and abstract signature facts', async () => {
  const expected = [
    ['func', '(x: string) => number'],
    ['ctor', 'new (x: number) => { value: string; }'],
    ['generic', '<T>(x: T) => T'],
    ['abstractCtor', 'abstract new (x: number) => { value: string; }'],
  ];
  for (const config of configs) for (const [name, summary] of expected) {
    await fresh(config, ({ checker, file }) => {
      const own = declaration(file, name);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(own.name));
      assert.equal(type.isErrorType(), false);
      assert.equal(checker.typeToString(type), summary);
    });
  }
});

test('native alias summary spelling and decoded alias names are distinct facts for every anonymous type syntax', async () => {
  for (const config of configs) for (const [name, decoded, summary] of [
    ['functionAlias', 'Fn', String.raw`\u0046n`],
    ['constructorAlias', 'Ctor', String.raw`\u0043tor`],
    ['literalAlias', 'Literal', String.raw`\u004citeral`],
  ]) {
    await fresh(config, ({ native, checker, file }) => {
      const own = declaration(file, name);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(own.name));
      assert.equal(type.isErrorType(), false);
      assert.equal(type.flags, native.TypeFlags.Object);
      assert.equal(checker.typeToString(type), summary);
      assert.equal(type.getAliasSymbol().name, decoded);
    });
  }
});

test('native merged alias prints its first canonical declaration while retaining the decoded alias name', async () => {
  for (const config of configs) for (const [name, decoded, summary, declarations] of [
    ['first', 'Alias', String.raw`\u0041lias`, [['VariableDeclaration', 'const ', String.raw`\u0041lias`], ['TypeAliasDeclaration', 'type ', 'Alias']]],
    ['second', 'Before', String.raw`\u0042efore`, [['TypeAliasDeclaration', 'type ', String.raw`\u0042efore`], ['VariableDeclaration', 'const ', 'Before']]],
    ['third', 'FunctionAlias', String.raw`\u0046unctionAlias`, [['FunctionDeclaration', 'function ', String.raw`\u0046unctionAlias`], ['TypeAliasDeclaration', 'type ', 'FunctionAlias']]],
    ['fourth', 'Plain', 'Plain', [['TypeAliasDeclaration', 'type ', 'Plain'], ['VariableDeclaration', 'const ', String.raw`\u0050lain`]]],
  ]) {
    await fresh(config, ({ native, project, checker, file }) => {
      const own = declaration(file, name);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(own.name));
      assert.equal(type.isErrorType(), false);
      assert.equal(checker.typeToString(type), summary);
      const symbol = type.getAliasSymbol();
      assert.equal(symbol.name, decoded);
      assert.deepEqual(symbol.declarations.map(handle => {
        const node = handle.resolve(project);
        return [native.SyntaxKind[node.kind], node.name.getStart(file), node.name.end,
          file.text.slice(node.name.getStart(file), node.name.end)];
      }), declarations.map(([kind, prefix, raw]) => {
        const start = file.text.indexOf(prefix + raw) + prefix.length;
        assert.ok(start >= prefix.length, raw);
        return [kind, start, start + raw.length, raw];
      }));
    }, 'written-names.ts');
  }
});

test('native written-name order is shared by interface, namespace and recursive function consumers', async () => {
  for (const config of configs) for (const [name, summary, kinds] of [
    ['ifaceFirst', String.raw`\u0049face`, ['VariableDeclaration', 'InterfaceDeclaration']],
    ['ifaceSecond', String.raw`\u0042ack`, ['InterfaceDeclaration', 'VariableDeclaration']],
    ['ifaceThird', String.raw`\u0043allableNamed`, ['FunctionDeclaration', 'InterfaceDeclaration']],
    ['ifaceFourth', 'CallableAfter', ['FunctionDeclaration', 'InterfaceDeclaration']],
    ['namespaceFirst', String.raw`\u004doduleOnly`, ['ModuleDeclaration', 'TypeAliasDeclaration']],
    ['namespaceAfter', String.raw`\u004doduleBefore`, ['TypeAliasDeclaration', 'ModuleDeclaration']],
    ['recursiveValue', '() => typeof Recursive', ['FunctionDeclaration', 'InterfaceDeclaration']],
  ]) {
    await fresh(config, ({ native, project, checker, file }) => {
      const own = declaration(file, name);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(own.name));
      assert.equal(type.isErrorType(), false);
      assert.equal(checker.typeToString(type), summary);
      const symbol = type.getAliasSymbol() ?? type.getSymbol();
      assert.deepEqual(symbol.declarations.map(handle => native.SyntaxKind[handle.resolve(project).kind]), kinds);
    }, 'written-names.ts');
  }
});
