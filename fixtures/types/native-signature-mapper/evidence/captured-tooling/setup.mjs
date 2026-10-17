import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root, '.cc/temp/types-signature-default');
const exact = (text, before, after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before, after);
};
for (const name of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
  fs.copyFileSync(path.join(root, 'fixtures/types/compiler-hook', name), path.join(here, name));
let test = fs.readFileSync(path.join(here, 'oracle-test.go'), 'utf8');
test = exact(test, '\t\toutput["beforeSummary"] = guardObservation(c)', `
    output["beforeSummary"] = guardObservation(c)
    signatures := c.GetSignaturesOfType(typ, checker.SignatureKindCall)
    if len(signatures) != 1 { t.Fatalf("expected one native call signature, found %d", len(signatures)) }
    signature := signatures[0]
    output["signatureId"] = signature.Id()
    output["signatureTargetId"] = nil
    if signature.Target() != nil { output["signatureTargetId"] = signature.Target().Id() }
    parameters := signature.TypeParameters()
    output["beforeDefaults"] = guardObservation(c)
    defaults := make([]map[string]any, len(parameters))
    for index, parameter := range parameters {
      resolvedDefault := c.GetDefaultFromTypeParameter(parameter)
      observation := map[string]any{"parameterId": parameter.Id(), "absent": resolvedDefault == nil}
      if resolvedDefault != nil {
        observation["id"] = resolvedDefault.Id()
        observation["errorType"] = c.AgentOracleIsErrorType(resolvedDefault)
        observation["summary"] = c.TypeToString(resolvedDefault)
        observation["flags"] = resolvedDefault.Flags()
      }
      defaults[index] = observation
    }
    output["signatureDefaults"] = defaults
    output["afterDefaults"] = guardObservation(c)`);
fs.writeFileSync(path.join(here, 'oracle-test.go'), test);
let oracle = fs.readFileSync(path.join(root, 'tools/type-oracle.mjs'), 'utf8');
for (const name of ['semantic-oracle.mjs', 'vendor-typescript-libs.mjs', 'navigation-oracle.mjs'])
  oracle = exact(oracle, `from './${name}'`, `from ${JSON.stringify(pathToFileURL(path.join(root, 'tools', name)).href)}`);
oracle = exact(oracle, "new URL('../fixtures/types/compiler-hook/', import.meta.url)",
  `new URL(${JSON.stringify(pathToFileURL(here + '/').href)})`);
oracle = exact(oracle, 'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,',
  'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, signatureId: raw.signatureId, signatureTargetId: raw.signatureTargetId, signatureDefaults: raw.signatureDefaults, beforeDefaults: raw.beforeDefaults, afterDefaults: raw.afterDefaults,');
fs.writeFileSync(path.join(here, 'oracle.mjs'), oracle);
