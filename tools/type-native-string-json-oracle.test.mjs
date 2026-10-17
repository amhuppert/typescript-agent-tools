import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {readFixture, runFixture} from './json-probes.mjs';
import {loadSemanticApi, lineStarts, position} from './semantic-oracle.mjs';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-string-json/cases.json', import.meta.url));
test('native cooked config/JSON strings and strict manifest boundary', async () => {
  const semantic = await loadSemanticApi();
  const differences = runFixture(semantic, fixture);
  const rows = readFixture(fixture).cases.map(entry => ({name:entry.name,
    native:differences.find(difference => difference.name === entry.name)?.native ?? entry.native}));
  fs.mkdirSync('.cc/temp', {recursive:true});
  fs.writeFileSync('.cc/temp/native-string-json.json', JSON.stringify(rows,null,2)+'\n');
  assert.equal(rows.length, 27);
  for (const row of rows) {
    const [family, variant] = row.name.split('-');
    const paired = ['pair','scalar'].includes(variant);
    if (family === 'config') {
      assert.deepEqual(row.native.diagnostics, paired ? [] : ['TS6053 -'], row.name);
      assert.deepEqual(row.native.program, paired ? ['main.ts','𐀀.ts'] : ['main.ts'], row.name);
    } else {
      assert.deepEqual(row.native.diagnostics, [], row.name);
      if (family === 'manifest') {
        assert.deepEqual(row.native.program, [`node_modules/pkg/${paired ? '𐀀' : 'index'}.d.ts (external)`, 'main.ts'], row.name);
      } else {
        assert.equal(row.native.names.length, 2, row.name);
        for (const name of row.native.names) {
          assert.equal(name.includes('data.json:1:9 PropertyAssignment'), paired, row.name);
        }
      }
    }
  }
  assert.deepEqual(differences, [], 'recorded native config/JSON/manifest observations');
});

test('native final types keep JSON keys and manifest paths exact', async () => {
  const spec = readFixture(fixture);
  const rows = [];
  for (const entry of spec.cases.filter(entry => !entry.name.startsWith('config-'))) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-string-json-types-'));
    try {
      const options = JSON.parse(spec.config);
      options.files.push('globals.d.ts');
      const globals = fs.readFileSync(new URL('../fixtures/types/native-generated-literal-context/globals.d.ts',import.meta.url),'utf8');
      for (const [file,text] of Object.entries({'tsconfig.json':JSON.stringify(options),'globals.d.ts':globals,...entry.files})) {
        fs.mkdirSync(path.dirname(path.join(root,file)), {recursive:true});
        fs.writeFileSync(path.join(root,file),text);
      }
      const text = entry.files['main.ts'];
      const at = {file:'main.ts',...position(lineStarts(text),text.indexOf('selected'))};
      const config = path.join(root,'tsconfig.json');
      const native = await compilerHookOracle(config,{at});
      const api = await typeOracle(config,{at,properties:false});
      assert.equal(native.compiler,'7.0.2');
      assert.deepEqual([native.summary,native.errorType],[api.summary,api.errorType],entry.name);
      assert.equal(native.errorType,false,entry.name);
      assert.deepEqual(native.beforeSummary.demandFailures,[],entry.name);
      rows.push({name:entry.name,summary:native.summary,location:api.location,origins:api.origins,
        codes:native.beforeSummary.diagnosticCodes});
    } finally { fs.rmSync(root,{recursive:true,force:true}); }
  }
  fs.writeFileSync('.cc/temp/native-string-json-types.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(rows.length,18);
  assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(path.dirname(fixture),'expected-types.json'),'utf8')));
});
