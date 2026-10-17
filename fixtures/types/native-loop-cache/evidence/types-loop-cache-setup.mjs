import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const root=process.cwd(),here=path.join(root,'.cc/temp/types-loop-cache');
fs.mkdirSync(here,{recursive:true});
const replace=(s,b,a)=>{if(s.split(b).length!==2)throw Error(`nonunique ${b}`);return s.replace(b,a);};
for(const name of ['checker-hook.go','ls-hook.go','oracle-test.go'])fs.copyFileSync(path.join(root,'fixtures/types/compiler-hook',name),path.join(here,name));
let test=fs.readFileSync(path.join(here,'oracle-test.go'),'utf8');
test=replace(test,'\t\toutput["beforeSummary"] = guardObservation(c)',`
        rawType := c.GetTypeOfSymbol(symbol)
        output["loopIdentity"] = map[string]any{"rawId": rawType.Id(), "actualId": typ.Id(), "sameIdentity": rawType == typ}
        output["beforeSummary"] = guardObservation(c)
`);
fs.writeFileSync(path.join(here,'oracle-test.go'),test);
let oracle=fs.readFileSync(path.join(root,'tools/type-oracle.mjs'),'utf8');
for(const name of ['semantic-oracle.mjs','vendor-typescript-libs.mjs','navigation-oracle.mjs'])oracle=replace(oracle,`from './${name}'`,`from ${JSON.stringify(pathToFileURL(path.join(root,'tools',name)).href)}`);
oracle=replace(oracle,"new URL('../fixtures/types/compiler-hook/', import.meta.url)",`new URL(${JSON.stringify(pathToFileURL(here+'/').href)})`);
oracle=replace(oracle,'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,','summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, loopIdentity: raw.loopIdentity,');
oracle=replace(oracle,"    const overlaidChecker = path.join(temporary, 'checker.go');",`    checkerSource = replaceExactly(checkerSource, '\\t\\tsaveFlowLoopStack := c.flowLoopStack', '\\t\\tAgentOracleCounters["expressionCacheReset"]++\\n\\t\\tsaveFlowLoopStack := c.flowLoopStack');
    const overlaidChecker = path.join(temporary, 'checker.go');`);
oracle=replace(oracle,"    const flowSource = replaceExactly(fs.readFileSync(flowPath, 'utf8')","    let flowSource = replaceExactly(fs.readFileSync(flowPath, 'utf8')");
const patches=[
 ['func (c *Checker) getTypeAtFlowLoopLabel(f *FlowState, flow *ast.FlowNode) FlowType {','func (c *Checker) getTypeAtFlowLoopLabel(f *FlowState, flow *ast.FlowNode) FlowType {\n\tAgentOracleCounters["loopCalls"]++'],
 ['\tfor _, loopInfo := range c.flowLoopStack {','\tfor _, loopInfo := range c.flowLoopStack {\n\t\tif loopInfo.key == key && len(loopInfo.types) == 0 { AgentOracleCounters["loopEmptyActive"]++ }'],
 ['\t\tif loopInfo.key == key && len(loopInfo.types) != 0 {','\t\tif loopInfo.key == key && len(loopInfo.types) != 0 {\n\t\t\tAgentOracleCounters["loopActiveHit"]++'],
 ['\t\t\tc.flowLoopStack = append(c.flowLoopStack, FlowLoopInfo{key: key, types: antecedentTypes})','\t\t\tc.flowLoopStack = append(c.flowLoopStack, FlowLoopInfo{key: key, types: antecedentTypes})\n\t\t\tAgentOracleCounter("loopStackDepth", len(c.flowLoopStack))'],
 ['\t\t\tif cached := c.flowLoopCache[key]; cached != nil {\n\t\t\t\treturn FlowType{t: cached}','\t\t\tif cached := c.flowLoopCache[key]; cached != nil {\n\t\t\t\tAgentOracleCounters["loopRestartPickup"]++\n\t\t\t\treturn FlowType{t: cached}'],
 ['\tif firstAntecedentType.incomplete {','\tif firstAntecedentType.incomplete {\n\t\tAgentOracleCounters["loopFirstIncomplete"]++'],
 ['\tc.flowLoopCache[key] = result','\tAgentOracleCounters["loopCompletedInstall"]++\n\tc.flowLoopCache[key] = result'],
];
oracle=replace(oracle,"    const overlaidFlow = path.join(temporary, 'flow.go');",patches.map(([b,a])=>`    flowSource = replaceExactly(flowSource, ${JSON.stringify(b)}, ${JSON.stringify(a)});`).join('\n')+"\n    flowSource = replaceExactly(flowSource, \"\\treturn c.getNarrowableTypeForReference(c.getAssignedType(flow.Node), f.reference, CheckModeNormal)\", \"\\tassigned := c.getAssignedType(flow.Node)\\n\\tif c.isErrorType(assigned) { AgentOracleCounters[\\\"loopAssignedErrorType\\\"]++ }\\n\\treturn c.getNarrowableTypeForReference(assigned, f.reference, CheckModeNormal)\");\n    const overlaidFlow = path.join(temporary, 'flow.go');");
oracle=replace(oracle, "    const overlaidChecker = path.join(temporary, 'checker.go');", "    checkerSource = replaceExactly(checkerSource, \"\\t\\t\\tlinks.resolvedSignature = cached\", \"\\t\\t\\tAgentOracleCounters[\\\"loopCallCacheSuppression\\\"]++\\n\\t\\t\\tlinks.resolvedSignature = cached\");\n    const overlaidChecker = path.join(temporary, 'checker.go');");
fs.writeFileSync(path.join(here,'oracle.mjs'),oracle);
