import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const root=process.cwd(), here=path.join(root,'.cc/temp/types-setter-return');
const exact=(s,b,a)=>{if(s.split(b).length!==2)throw Error(`nonunique ${b}`);return s.replace(b,a);};
for(const name of ['checker-hook.go','ls-hook.go','oracle-test.go'])fs.copyFileSync(path.join(root,'fixtures/types/compiler-hook',name),path.join(here,name));
let test=fs.readFileSync(path.join(here,'oracle-test.go'),'utf8');
test=exact(test,'\t\toutput["beforeSummary"] = guardObservation(c)',`
        output["beforeSummary"] = guardObservation(c)
        declaration := node.Parent
        if declaration.Kind != ast.KindSetAccessor { t.Fatalf("expected setter, got %v", declaration.Kind) }
        signature := c.GetSignatureFromDeclaration(declaration)
        beforeReturn := guardObservation(c)
        returned := c.GetReturnTypeOfSignature(signature)
        output["setterReturn"] = map[string]any{"id": returned.Id(), "summary": c.TypeToString(returned), "flags": returned.Flags(), "errorType": c.AgentOracleIsErrorType(returned), "beforeReturn": beforeReturn, "afterReturn": guardObservation(c)}
`);
fs.writeFileSync(path.join(here,'oracle-test.go'),test);
let oracle=fs.readFileSync(path.join(root,'tools/type-oracle.mjs'),'utf8');
for(const name of ['semantic-oracle.mjs','vendor-typescript-libs.mjs','navigation-oracle.mjs'])oracle=exact(oracle,`from './${name}'`,`from ${JSON.stringify(pathToFileURL(path.join(root,'tools',name)).href)}`);
oracle=exact(oracle,"new URL('../fixtures/types/compiler-hook/', import.meta.url)",`new URL(${JSON.stringify(pathToFileURL(here+'/').href)})`);
oracle=exact(oracle,'summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,','summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, setterReturn: raw.setterReturn,');
fs.writeFileSync(path.join(here,'oracle.mjs'),oracle);
fs.writeFileSync(path.join(here,'cases.ts'),`export type Bodyless = { set /*bodyless*/value(next: number); };
export type Paired = { get value(): string; set /*paired*/value(next: Missing); };
export class Implemented { set /*implemented*/value(next: number) {} }
`);
for(const [name,strict] of [['strict',true],['loose',false]]) fs.writeFileSync(path.join(here,name+'.json'),JSON.stringify({compilerOptions:{strict,noLib:true,noEmit:true,target:'esnext',module:'esnext'},files:['cases.ts']},null,2)+'\n');
