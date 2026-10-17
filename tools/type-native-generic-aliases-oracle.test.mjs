import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generic-aliases/', import.meta.url));
for (const config of ['tsconfig.json', 'loose.json']) {
  test(`native generic alias lifecycle, defaults and lazy values (${config})`, async () => {
    const native = await loadSemanticApi();
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject:path.join(root,config)});
      const project = snapshot.getProject(path.join(root,config));
      const checker = project.checker;
      for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),
        project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics,[]);
      const describe = type => ({summary:checker.typeToString(type),error:type.isErrorType()});
      const observations = {};
      for (const filename of ['cases.ts','imported.ts']) {
        const text = fs.readFileSync(path.join(root,filename),'utf8');
        const file = project.program.getSourceFile(path.join(root,filename));
        for (const match of text.matchAll(/\/\*#(\w+)\*\//g)) {
          const node = native.getTokenAtPosition(file,match.index+match[0].length);
          const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
          const before = describe(type);
          const properties = type.isErrorType() ? [] : checker.getPropertiesOfType(type).map(symbol => ({name:symbol.name,...describe(checker.getTypeOfSymbol(symbol))}));
          observations[match[1]] = {...before,properties};
          assert.deepEqual(describe(type),before,`stable after child reads: ${match[1]}`);
        }
      }
      const healthy = (summary, properties = []) => ({summary,error:false,properties});
      const property = (name,summary,error = false) => ({name,summary,error});
      const recovery = {summary:'any',error:true,properties:[]};
      assert.deepEqual(observations, {
        scalar: healthy('"value"'), choice: healthy('Choice<"a"> | Choice<"z">'),
        pair: healthy('Pair<"left", 42>', [property('left','"left"'),property('right','42')]),
        defaulted: healthy('Defaulted<string, string>', [property('left','string'),property('right','string')]),
        dependent: healthy('Defaulted<"value", "value">', [property('left','"value"'),property('right','"value"')]),
        ignoredDefault: healthy('number'), ignoredSupplied: healthy('number'), constraint: healthy('42'),
        forward: healthy('Forward<"value">'),
        recursive: healthy('Recursive<string>', [property('value','string'),property('next',config === 'tsconfig.json' ? 'Recursive<string> | undefined' : 'Recursive<string>')]),
        lazyMissing: healthy('MissingProperty<number>', [property('actual','number'),property('missing','MissingType',true)]),
        tooFew: recovery, tooMany: recovery, circular: recovery,
        imported: healthy('Exported<"value">', [property('exported','"value"')]),
        nestedImported: healthy('Exported<Exported<Exported<string>>>',
          [property('exported', 'Exported<Exported<string>>')]),
      });
    } finally { try {snapshot?.dispose();} finally {api.close();} }
  });
}
