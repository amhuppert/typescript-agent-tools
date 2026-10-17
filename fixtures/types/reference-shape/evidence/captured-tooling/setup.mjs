import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root, '.cc/temp/types-reference-shape');
const exact = (text, before, after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, '\t\toutput["beforeSummary"] = guardObservation(c)', `
    output["beforeSummary"] = guardObservation(c)
    output["shape"] = c.AgentReferenceShape(typ)
    output["afterShape"] = guardObservation(c)`);
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
fs.appendFileSync(path.join(here, 'checker-hook.go'), `
// Scratch-only native whole inventory demand, preserving its original order.
func (c *Checker) AgentReferenceShape(t *Type) map[string]any {
  calls := c.getSignaturesOfType(t, SignatureKindCall)
  constructs := c.getSignaturesOfType(t, SignatureKindConstruct)
  indexes := c.getIndexInfosOfType(t)
  indexErrors := make([]bool, len(indexes))
  for i, index := range indexes { indexErrors[i] = c.isErrorType(index.valueType) }
  return map[string]any{"calls": len(calls), "constructs": len(constructs), "indexes": len(indexes), "indexErrors": indexErrors}
}
`);
let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
for (const name of ['semantic-oracle.mjs', 'vendor-typescript-libs.mjs', 'navigation-oracle.mjs'])
  oracle = exact(oracle, `from './${name}'`, `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools', name)).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)", `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,',
  'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, shape: raw.shape, afterShape: raw.afterShape,');
fs.writeFileSync(path.join(here, 'oracle.mjs'), oracle);
