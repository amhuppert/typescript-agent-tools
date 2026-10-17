import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root = process.cwd();
const here = path.join(root,'.cc/temp/types-facts');
fs.mkdirSync(here,{recursive:true});
const exact = (text,before,after) => {
  if (text.split(before).length !== 2) throw new Error(`Nonunique anchor: ${before}`);
  return text.replace(before,after);
};
for (const name of ['checker-hook.go','ls-hook.go','oracle-test.go']) fs.copyFileSync(path.join(root,'fixtures/types/compiler-hook',name),path.join(here,name));
let test = fs.readFileSync(path.join(here,'oracle-test.go'),'utf8');
test = exact(test,'\t\toutput["beforeSummary"] = guardObservation(c)', '\t\toutput["beforeSummary"] = guardObservation(c)\n\t\toutput["typeFacts"] = c.AgentTypeFacts(typ)');
fs.writeFileSync(path.join(here,'oracle-test.go'),test);
fs.appendFileSync(path.join(here,'checker-hook.go'), `
// Scratch-only direct observations; native masks, thresholds and identities unchanged.
func (c *Checker) AgentTypeFacts(t *Type) map[string]any {
  result := map[string]any{"all": uint32(c.getTypeFacts(t, TypeFactsAll)), "flags": uint32(t.flags)}
  masks := []struct { name string; mask TypeFacts }{
    {"truthy",TypeFactsTruthy},{"falsy",TypeFactsFalsy},
    {"typeofString",TypeFactsTypeofEQString},{"typeofNumber",TypeFactsTypeofEQNumber},
    {"typeofBigint",TypeFactsTypeofEQBigInt},{"typeofBoolean",TypeFactsTypeofEQBoolean},
    {"typeofSymbol",TypeFactsTypeofEQSymbol},{"typeofObject",TypeFactsTypeofEQObject},
    {"typeofFunction",TypeFactsTypeofEQFunction},{"notNumber",TypeFactsTypeofNENumber},
    {"notUndefined",TypeFactsNEUndefined},{"notNull",TypeFactsNENull},{"notNullable",TypeFactsNEUndefinedOrNull},
    {"isUndefined",TypeFactsIsUndefined},{"isNull",TypeFactsIsNull},
  }
  rows := []map[string]any{}
  for _, entry := range masks {
    filtered := c.getTypeWithFacts(t, entry.mask)
    adjusted := c.getAdjustedTypeWithFacts(t, entry.mask)
    rows = append(rows, map[string]any{"name":entry.name,"mask":uint32(entry.mask),
      "facts":uint32(c.getTypeFacts(t,entry.mask)),
      "filtered":map[string]any{"same":filtered==t,"flags":uint32(filtered.flags),"summary":c.TypeToString(filtered),"errorType":c.isErrorType(filtered)},
      "adjusted":map[string]any{"same":adjusted==t,"flags":uint32(adjusted.flags),"summary":c.TypeToString(adjusted),"errorType":c.isErrorType(adjusted)}})
  }
  result["masks"] = rows
  return result
}
`);
let oracle = fs.readFileSync(path.join(root,'tools/type-oracle.mjs'),'utf8');
for (const name of ['semantic-oracle.mjs','vendor-typescript-libs.mjs','navigation-oracle.mjs']) oracle = exact(oracle,`from './${name}'`,`from ${JSON.stringify(pathToFileURL(path.join(root,'tools',name)).href)}`);
oracle = exact(oracle,"new URL('../fixtures/types/compiler-hook/', import.meta.url)",`new URL(${JSON.stringify(pathToFileURL(here+'/').href)})`);
oracle = exact(oracle,'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,','summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, typeFacts: raw.typeFacts,');
fs.writeFileSync(path.join(here,'oracle.mjs'),oracle);
