import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

const root=process.cwd(), here=path.join(root,'.cc/temp/types-narrowable-reference');
fs.mkdirSync(here,{recursive:true});
const replace=(text,before,after)=>{if(text.split(before).length!==2)throw Error(`nonunique anchor ${before}`);return text.replace(before,after);};
for(const name of ['checker-hook.go','ls-hook.go','oracle-test.go'])fs.copyFileSync(path.join(root,'fixtures/types/compiler-hook',name),path.join(here,name));
fs.appendFileSync(path.join(here,'checker-hook.go'),`
// Scratch observation only: exact native reference question and mode.
func (c *Checker) AgentOracleNarrowableReference(typ *Type, node *ast.Node, inferential bool) *Type {
    mode := CheckModeNormal
    if inferential { mode = CheckModeInferential }
    return c.getNarrowableTypeForReference(typ, node, mode)
}
`);
let test=fs.readFileSync(path.join(here,'oracle-test.go'),'utf8');
test=replace(test,'\tInspectArrayElement bool','\tNarrowMode string\n\tInspectArrayElement bool');
test=replace(test,'\t\toutput["beforeSummary"] = guardObservation(c)',`
        baseline := guardObservation(c)
        raw := c.GetTypeOfSymbol(symbol)
        narrowed := c.AgentOracleNarrowableReference(raw, node, input.NarrowMode == "inferential")
        afterNarrow := guardObservation(c)
        output["narrowable"] = map[string]any{"rawId": raw.Id(), "rawFlags": raw.Flags(), "narrowedId": narrowed.Id(), "narrowedFlags": narrowed.Flags(), "sameIdentity": raw == narrowed, "errorType": c.AgentOracleIsErrorType(narrowed), "before": baseline, "after": afterNarrow, "summary": c.TypeToString(narrowed)}
        output["beforeSummary"] = guardObservation(c)
`);
fs.writeFileSync(path.join(here,'oracle-test.go'),test);
let oracle=fs.readFileSync(path.join(root,'tools/type-oracle.mjs'),'utf8');
for(const name of ['semantic-oracle.mjs','vendor-typescript-libs.mjs','navigation-oracle.mjs'])oracle=replace(oracle,`from './${name}'`,`from ${JSON.stringify(pathToFileURL(path.join(root,'tools',name)).href)}`);
oracle=replace(oracle,"new URL('../fixtures/types/compiler-hook/', import.meta.url)",`new URL(${JSON.stringify(pathToFileURL(here+'/').href)})`);
oracle=replace(oracle,'inspectArrayElement: request.inspectArrayElement === true,','inspectArrayElement: request.inspectArrayElement === true, narrowMode: request.narrowMode,');
oracle=replace(oracle,'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,','summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, narrowable: raw.narrowable,');
fs.writeFileSync(path.join(here,'oracle.mjs'),oracle);
