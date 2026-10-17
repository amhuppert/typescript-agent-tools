import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generic-aliases/', import.meta.url));
test('native anonymous instantiation observes first and all symbol declarations plus unnamed syntax roles', async () => {
  const native = await loadSemanticApi();
  const api = new native.API({tsserverPath:native.binary,cwd:root});
  let snapshot;
  try {
    const config = path.join(root,'contexts.json');
    snapshot = api.updateSnapshot({openProject:config});
    const project = snapshot.getProject(config);
    const checker = project.checker;
    const source = project.program.getSourceFile(path.join(root,'contexts.ts'));
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics,[]);
    const typeAt = marker => {
      const tag = `/*#${marker}*/`;
      const token = native.getTokenAtPosition(source, source.text.indexOf(tag)+tag.length);
      return checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));
    };
    const parameter = checker.getSymbolAtLocation(source.statements[0].typeParameters[1].name);
    const method = checker.getSymbolAtLocation(source.statements[0].members[0].name);
    assert.equal(parameter.id,method.id);
    assert.deepEqual(method.declarations.map(node=>node.kind),[native.SyntaxKind.TypeParameter,native.SyntaxKind.MethodSignature]);
    const methods = typeAt('methods').getTypes().map(type=>checker.getTypeOfSymbol(checker.getPropertiesOfType(type).find(symbol=>symbol.name==='B')));
    assert.deepEqual(methods.map(type=>checker.typeToString(type)),['() => number','() => number']);
    assert.notEqual(methods[0].id,methods[1].id,'the first parameter declaration references outer A');
    for (const [marker,summary] of [['callable','Callable<string>'],['constructor','Constructor<number>']]) {
      const type = typeAt(marker);
      assert.equal(checker.typeToString(type),summary);
      assert.equal(type.isErrorType(),false);
    }
  } finally { try {snapshot?.dispose();} finally {api.close();} }
});
