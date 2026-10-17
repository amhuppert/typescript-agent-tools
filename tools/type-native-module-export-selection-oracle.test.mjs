import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-module-export-selection/', import.meta.url));
const source = fs.readFileSync(path.join(root,'cases.cts'),'utf8');
// Module names spell the fixture's absolute directory; compare them as
// `<fixture>/…`, wherever the repository is checked out.
const located = rows => JSON.parse([root, fs.realpathSync(root) + path.sep].reduce(
  (text, directory) => text.replaceAll(directory, '<fixture>/'), JSON.stringify(rows)));
for (const profile of ['node20','nodenext']) {
  test(`native alias export selection and module-star links (${profile})`, async () => {
    const rows = [];
    const config = path.join(root,`${profile}.json`);
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file:'cases.cts',...position(lineStarts(source),marker.index+marker[0].length)};
      const native = await compilerHookOracle(config,{at,inspectDiagnosticCache:true});
      const api = await typeOracle(config,{at,properties:false});
      assert.equal(native.compiler,'7.0.2');
      assert.deepEqual([native.summary,native.errorType],[api.summary,api.errorType],marker[1]);
      rows.push({name:marker[1],summary:native.summary,errorType:native.errorType,
        location:api.location,origins:api.origins,codes:native.beforeSummary.diagnosticCodes,
        demandFailures:native.beforeSummary.demandFailures,acquisitions:native.beforeSummary.displayAcquisitions ?? []});
      fs.mkdirSync('.cc/temp',{recursive:true});
      fs.writeFileSync(`.cc/temp/native-module-export-selection-${profile}.json`,JSON.stringify(located(rows),null,2)+'\n');
    }
    assert.equal(rows.length,12);
    assert.deepEqual(located(rows),JSON.parse(fs.readFileSync(path.join(root,`expected-${profile}.json`),'utf8')));
  });
}
