import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import fs from 'node:fs';
import {loadSemanticApi, lineStarts, position} from './semantic-oracle.mjs';
import {typeOracle} from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-property-clone/', import.meta.url));

test('native merged method clones preserve transient reads and create fresh types for canonical this members', async () => {
  const native = await loadSemanticApi();
  for (const profile of ['tsconfig.json', 'loose.json', 'exact.json']) for (const ownerName of ['Generic', 'Interface'])
    for (const [methodName, propertyName, cloneOwnsType] of [
      ['use', 'method', false], ['useNumber', 'method', false],
      ['use', 'plain', false], ['useNumber', 'plain', false],
      ['useThis', 'plain', true], ['useThis', 'method', true],
    ]) {
    const config = path.join(root, profile);
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const checker = project.checker;
      const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
      const declaration = source.statements.find(node => node.name?.text === ownerName);
      const method = declaration.members.find(node => node.name?.text === propertyName);
      const use = declaration.members.find(node => node.name?.text === methodName);
      const parameterType = checker.getTypeAtLocation(use.parameters[0]);
      assert.equal(parameterType.isUnionType(), true);
      const property = checker.getPropertyOfType(parameterType, propertyName);
      const original = checker.getSymbolAtLocation(method.name);
      assert.ok(property);
      assert.notEqual(property.id, original.id, 'the union actually created a distinct symbol');
      const type = checker.getTypeOfSymbol(property);
      const observed = {profile, ownerName, methodName, propertyName, property: {id: property.id, flags:property.flags, checkFlags:property.checkFlags},
        original: {id:original.id, flags:original.flags, checkFlags:original.checkFlags},
        typeSymbol: {id:type.getSymbol()?.id, flags:type.getSymbol()?.flags, checkFlags:type.getSymbol()?.checkFlags},
        constituents:parameterType.getTypes().map(constituent => {
          const member = checker.getPropertyOfType(constituent, propertyName);
          return {summary:checker.typeToString(constituent), member:member.id, flags:member.flags, checks:member.checkFlags};
        })};
      assert.equal((observed.constituents[0].flags & native.SymbolFlags.Transient) === 0, cloneOwnsType, JSON.stringify(observed));
      assert.equal(type.getSymbol()?.id, cloneOwnsType ? property.id : original.id,
        JSON.stringify(observed));
      assert.equal(checker.typeToString(type), '() => number');
    } finally {
      try { snapshot?.dispose(); } finally { api.close(); }
    }
  }
});


test('native final accesses expose canonical and transient clone method types', async () => {
  const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json'])
    for (const [marker, summary] of [
      ['classMethod', '() => number'], ['classPlain', '() => number'], ['classTransient', '() => number'],
      ['interfaceMethod', '() => number'], ['interfacePlain', '() => number'],
      ['classField', 'number'], ['interfaceField', 'number'], ['classAccessor', 'number'], ['interfaceAccessor', 'number'],
      ['requiredFirst', 'number'], ['optionalFirst', config === 'loose.json' ? 'number' : 'number | undefined'],
      ['classOptional', config === 'loose.json' ? '() => number' : '(() => number) | undefined'],
      ['interfaceOptional', config === 'loose.json' ? '() => number' : '(() => number) | undefined'],
    ]) {
      const label = `/*#${marker}*/`;
      const result = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(label) + label.length)},
        properties: true,
      });
      assert.equal(result.summary, summary, `${config}/${marker}`);
      assert.equal(result.errorType, false, `${config}/${marker}`);
    }
});
