import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root, '.cc/temp/types-array-classification');
const exact = (text, before, after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, '\t\toutput["beforeSummary"] = guardObservation(c)', `
    output["beforeSummary"] = guardObservation(c)
    output["arrayClassification"] = c.AgentArrayClassification(typ)`);
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
fs.appendFileSync(path.join(here, 'checker-hook.go'), `
// Scratch-only observation of the actual native predicates; no operations added.
func (c *Checker) AgentArrayClassification(t *Type) map[string]any {
  reference := t.objectFlags&ObjectFlagsReference != 0
  return map[string]any{"isArray": c.isArrayType(t), "nativeReadonly": c.isReadonlyArrayType(t),
    "targetIsArray": reference && t.Target() == c.globalArrayType,
    "targetIsReadonly": reference && t.Target() == c.globalReadonlyArrayType}
}
`);

let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
for (const name of ['semantic-oracle.mjs', 'vendor-typescript-libs.mjs', 'navigation-oracle.mjs'])
  oracle = exact(oracle, `from './${name}'`, `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools', name)).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)", `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,',
  'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, arrayClassification: raw.arrayClassification,');
fs.writeFileSync(path.join(here, 'oracle.mjs'), oracle);
