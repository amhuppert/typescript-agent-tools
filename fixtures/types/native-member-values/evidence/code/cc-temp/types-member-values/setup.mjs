import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root, '.cc/temp/types-member-values');
const exact = (text, before, after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
fs.appendFileSync(path.join(here, 'checker-hook.go'), `

// Disposable native member question controls. No delivered implementation.
func (c *Checker) AgentOracleMemberValue(t *Type, name string) map[string]any {
  properties := c.GetPropertiesOfType(t)
  names := make([]string, len(properties))
  var chosen *ast.Symbol
  for index, property := range properties {
    names[index] = property.Name
    if property.Name == name { chosen = property }
  }
  if chosen == nil { panic("requested native member is absent: " + name) }
  describe := func(t *Type) map[string]any {
    parts := []*Type{t}
    if t.Flags() & TypeFlagsUnion != 0 { parts = t.Types() }
    containsMissing := false
    alternatives := make([]map[string]any, len(parts))
    for index, part := range parts {
      containsMissing = containsMissing || part == c.missingType
      alternatives[index] = map[string]any{"id":part.Id(), "summary":c.TypeToString(part),
        "flags":part.Flags(), "errorType":c.isErrorType(part), "missing":part == c.missingType}
    }
    return map[string]any{"id":t.Id(), "summary":c.TypeToString(t), "flags":t.Flags(),
      "errorType":c.isErrorType(t), "containsMissing":containsMissing, "alternatives":alternatives}
  }
  output := map[string]any{"names":names, "name":chosen.Name, "flags":chosen.Flags,
    "readonly":c.isReadonlySymbol(chosen), "modifierFlags":getDeclarationModifierFlagsFromSymbol(chosen)}
  read := c.getTypeOfSymbol(chosen)
  output["read"] = describe(read)
  output["afterReadDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  output["nonMissingRead"] = describe(c.getNonMissingTypeOfSymbol(chosen))
  output["afterNonMissingReadDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  output["write"] = describe(c.getWriteTypeOfSymbol(chosen))
  output["afterWriteDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  locations := make([]map[string]any, 0, len(chosen.Declarations))
  for _, declaration := range chosen.Declarations {
    name := ast.GetNameOfDeclaration(declaration)
    if name != nil {
      locations = append(locations, map[string]any{"kind":declaration.Kind.String(),
        "pos":name.Pos(), "end":name.End(), "type":describe(c.GetTypeOfSymbolAtLocation(chosen, name))})
    }
  }
  output["declarations"] = locations
  return output
}
`);
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, '\tInspectArrayElement bool', '\tInspectArrayElement bool\n\tMemberName string');
test = exact(test, '\tif len(fatal) != 0 {', `
  // This bounded probe retains the one diagnostic-only option contradiction.
  // All parse/bind/syntax and other program failures still reject the setup.
  options := config.CompilerOptions()
  diagnosticOnlyCombination := len(fatal) == 1 && fatal[0].Code() == 5052 &&
    options.ExactOptionalPropertyTypes.IsTrue() && !options.GetStrictOptionValue(options.StrictNullChecks)
  if len(fatal) != 0 && !diagnosticOnlyCombination {`);
test = exact(test, '\t\toutput["beforeSummary"] = guardObservation(c)',
  '\t\toutput["beforeSummary"] = guardObservation(c)\n\t\toutput["nativeProgramDiagnosticCodes"] = make([]int32, len(fatal))\n\t\tfor index, diagnostic := range fatal { output["nativeProgramDiagnosticCodes"].([]int32)[index] = diagnostic.Code() }\n\t\toutput["memberValue"] = c.AgentOracleMemberValue(typ, input.MemberName)\n\t\toutput["afterMemberValue"] = guardObservation(c)');
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
oracle = exact(oracle, 'if (fatal.length) throw new Error(',
  `const rejected = fatal.filter(d => !(d.code === 5052 && d.text.includes('exactOptionalPropertyTypes') && d.text.includes('strictNullChecks')));
    if (rejected.length) throw new Error(`);
oracle = exact(oracle, 'return action({ semantic, project, configPath, root:',
  'return action({ observedPreflightDiagnostics: fatal, semantic, project, configPath, root:');
oracle = exact(oracle, 'return { files, root: context.root, config:',
  'return { observedPreflightDiagnostics: context.observedPreflightDiagnostics, files, root: context.root, config:');
oracle = exact(oracle, "return { compiler: '7.0.2', sourceCommit: GO_SOURCE_COMMIT, history, status: 'observed', inputFiles, inputHashes,",
  "return { compiler: '7.0.2', sourceCommit: GO_SOURCE_COMMIT, history, status: 'observed', inputFiles, inputHashes, observedPreflightDiagnostics: input.observedPreflightDiagnostics,");
for (const name of ['semantic-oracle.mjs', 'vendor-typescript-libs.mjs', 'navigation-oracle.mjs'])
  oracle = exact(oracle, `from './${name}'`, `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools', name)).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)",
  `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'inspectArrayElement: request.inspectArrayElement === true,',
  'inspectArrayElement: request.inspectArrayElement === true, memberName: request.memberName,');
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,',
  'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, memberValue: raw.memberValue, afterMemberValue: raw.afterMemberValue, nativeProgramDiagnosticCodes: raw.nativeProgramDiagnosticCodes,');
fs.writeFileSync(path.join(here, 'oracle.mjs'), oracle);
