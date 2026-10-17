import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const root = process.cwd();
const here = path.join(root, '.cc/temp/types-relation-limits');
const exact = (source, before, after) => {
  if (source.split(before).length !== 2) throw new Error(`Anchor not unique: ${before}`);
  return source.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
let hook = fs.readFileSync(path.join(here, 'checker-hook.go'), 'utf8');
hook += `
var AgentOracleResearchEvents = []map[string]any{}
func AgentOracleResearchEvent(name string, values map[string]any) {
  values["name"] = name
  AgentOracleResearchEvents = append(AgentOracleResearchEvents, values)
}
func (c *Checker) AgentOracleCurrentDiagnostics() []int32 {
  result := []int32{}
  for _, diagnostic := range c.diagnostics.GetDiagnostics() { result = append(result, diagnostic.Code()) }
  return result
}
`;
fs.writeFileSync(path.join(here, 'checker-hook.go'), hook);
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, 'type oracleGuardObservation struct {', 'type oracleGuardObservation struct {\n ResearchEvents []map[string]any `json:"researchEvents"`');
test = exact(test, 'return oracleGuardObservation{Counters:', 'return oracleGuardObservation{ResearchEvents: append([]map[string]any{}, checker.AgentOracleResearchEvents...), Counters:');
test = exact(test, 'output["beforeSummary"] = guardObservation()', 'output["beforeSummary"] = guardObservation()\n output["diagnosticCodes"] = c.AgentOracleCurrentDiagnostics()');
test = exact(test, 'output["diagnosticCodes"] = c.AgentOracleCurrentDiagnostics()', `output["diagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
 if c.IsArrayType(typ) {
   element := c.GetTypeArguments(typ)[0]
   count := 1
   if element.Flags()&checker.TypeFlagsUnion != 0 { count = len(element.Types()) }
   output["arrayElement"] = map[string]any{"errorType": element == c.GetErrorType(), "constituents": count}
 }`);
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
oracle = exact(oracle, "from './semantic-oracle.mjs'", `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools/semantic-oracle.mjs')).href)}`);
oracle = exact(oracle, "from './vendor-typescript-libs.mjs'", `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools/vendor-typescript-libs.mjs')).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)", `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'const overlaidChecker = path.join(temporary, \'checker.go\');', `
    checkerSource = replaceExactly(checkerSource, '\\t\\t\\t\\t\\t\\testimatedCount := (count / (length - i)) * length',
      '\\t\\t\\t\\t\\t\\testimatedCount := (count / (length - i)) * length\\n\\t\\t\\t\\t\\t\\tAgentOracleResearchEvent("subtype_estimate", map[string]any{"count": count, "length": length, "processed": length-i, "estimate": estimatedCount})');
    checkerSource = replaceExactly(checkerSource, '\\t\\t\\t\\t\\t\\tif estimatedCount > 1000000 {',
      '\\t\\t\\t\\t\\t\\tif estimatedCount > 1000000 {\\n\\t\\t\\t\\t\\t\\t\\tAgentOracleFailure("subtype_complexity_limit")');
    const relaterPath = path.join(sourceDirectory, 'internal/checker/relater.go');
    let relaterSource = fs.readFileSync(relaterPath, 'utf8');
    relaterSource = replaceExactly(relaterSource, '\\tr.relationCount = (16_000_000 - relation.size()) / 8',
      '\\tr.relationCount = (16_000_000 - relation.size()) / 8\\n\\tAgentOracleCounter("relationCacheSize", uint64(relation.size()))\\n\\tAgentOracleCounter("relationInitialAllowance", uint64(r.relationCount))');
    relaterSource = replaceExactly(relaterSource, '\\tresult := r.isRelatedToEx(source, target, RecursionFlagsBoth, errorNode != nil /*reportErrors*/, headMessage, IntersectionStateNone)',
      '\\tresult := r.isRelatedToEx(source, target, RecursionFlagsBoth, errorNode != nil /*reportErrors*/, headMessage, IntersectionStateNone)\\n\\tAgentOracleResearchEvent("relation_finished", map[string]any{"allowance": r.relationCount, "cacheSize": relation.size(), "result": int(result), "overflow": r.overflow})');
    relaterSource = replaceExactly(relaterSource, '\\tif r.relationCount <= 0 {',
      '\\tAgentOracleCounter("relationSourceDepth", uint64(len(r.sourceStack)))\\n\\tAgentOracleCounter("relationTargetDepth", uint64(len(r.targetStack)))\\n\\tAgentOracleCounter("relationAllowanceUsed", uint64(2_000_000-r.relationCount))\\n\\tif r.relationCount <= 0 {\\n\\t\\tAgentOracleFailure("relation_allowance_limit")');
    relaterSource = replaceExactly(relaterSource, '\\tif r.maybeKeysSet.Has(id) {',
      '\\tif r.maybeKeysSet.Has(id) {\\n\\t\\tAgentOracleResearchEvent("active_key_maybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    relaterSource = replaceExactly(relaterSource, '\\t\\tif r.maybeKeysSet.Has(broadestEquivalentId) {',
      '\\t\\tif r.maybeKeysSet.Has(broadestEquivalentId) {\\n\\t\\t\\tAgentOracleResearchEvent("constrained_key_maybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    relaterSource = replaceExactly(relaterSource, '\\tif len(r.sourceStack) == 100 || len(r.targetStack) == 100 {',
      '\\tif len(r.sourceStack) == 100 || len(r.targetStack) == 100 {\\n\\t\\tAgentOracleFailure("relation_depth_limit")');
    relaterSource = replaceExactly(relaterSource, '\\tif r.expandingFlags == ExpandingFlagsBoth {',
      '\\tif r.expandingFlags == ExpandingFlagsBoth {\\n\\t\\tAgentOracleResearchEvent("deeply_nested_maybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    const overlaidRelater = path.join(temporary, 'relater.go');
    fs.writeFileSync(overlaidRelater, relaterSource);
    attach(overlaidRelater, 'internal/checker/relater.go');
    const overlaidChecker = path.join(temporary, 'checker.go');`);
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary', 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, diagnosticCodes: raw.diagnosticCodes, arrayElement: raw.arrayElement');
fs.writeFileSync(path.join(here, 'research-oracle.mjs'), oracle);
console.log('Frozen research oracle and three hook fixture copies.');
