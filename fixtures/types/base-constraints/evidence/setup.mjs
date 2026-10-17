import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root, '.cc/temp/types-base-constraints');
const exact = (text, before, after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, '\t\toutput["beforeSummary"] = guardObservation(c)', '\t\toutput["beforeSummary"] = guardObservation(c)\n\t\toutput["baseConstraint"] = c.AgentBaseConstraint(typ)\n\t\toutput["afterBaseConstraint"] = guardObservation(c)');
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
fs.appendFileSync(path.join(here, 'checker-hook.go'), `
// Scratch-only native question sequence, preserving actual cache/sentinel identities.
func (c *Checker) AgentBaseConstraint(t *Type) map[string]any {
  describe := func(child *Type) any {
    if child == nil { return nil }
    return map[string]any{"summary":c.TypeToString(child),"flags":uint32(child.flags),"errorType":c.isErrorType(child),"same":child==t}
  }
  base := c.getBaseConstraintOfType(t)
  result := map[string]any{"base":describe(base)}
  resolved := c.getResolvedBaseConstraint(t,nil)
  sentinel := "type"
  if resolved == c.noConstraintType { sentinel = "noConstraint" }
  if resolved == c.circularConstraintType { sentinel = "circularConstraint" }
  result["resolvedKind"] = sentinel
  if t.flags&TypeFlagsTypeParameter != 0 {
    result["guarded"] = describe(c.getConstraintOfTypeParameter(t))
    result["raw"] = describe(c.getConstraintFromTypeParameter(t))
  }
  return result
}
`);
let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
for (const name of ['semantic-oracle.mjs', 'vendor-typescript-libs.mjs', 'navigation-oracle.mjs'])
  oracle = exact(oracle, `from './${name}'`, `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools', name)).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)", `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,',
  'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, baseConstraint: raw.baseConstraint, afterBaseConstraint: raw.afterBaseConstraint,');
oracle = exact(oracle, "    const overlaidChecker = path.join(temporary, 'checker.go');", `
    checkerSource = replaceExactly(checkerSource, '\\tidentity := getRecursionIdentity(t)\\n\\tif len(stack) < 10 || len(stack) < 50 && !slices.Contains(stack, identity) {',
      '\\tidentity := getRecursionIdentity(t)\\n\\tAgentOracleCounter("baseConstraintDepth", len(stack))\\n\\tif slices.Contains(stack, identity) { AgentOracleObservation("baseConstraintRepeat", map[string]any{"depth":len(stack)}) }\\n\\tif !(len(stack) < 10 || len(stack) < 50 && !slices.Contains(stack, identity)) { AgentOracleObservation("baseConstraintStop", map[string]any{"depth":len(stack),"repeat":slices.Contains(stack,identity)}) }\\n\\tif len(stack) < 10 || len(stack) < 50 && !slices.Contains(stack, identity) {');
    const overlaidChecker = path.join(temporary, 'checker.go');`);
fs.writeFileSync(path.join(here, 'oracle.mjs'), oracle);
