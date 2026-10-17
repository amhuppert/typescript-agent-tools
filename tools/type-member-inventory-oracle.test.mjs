import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/member-search-inventory/',import.meta.url));
const markers={
 'roles.ts':['memberModifier','parameterName','memberUse','shapeKey','shorthand','bindingKey','bindingShort','literalDeclaration','literalUse','computedUse'],
 'view.tsx':['jsxDeclaration','jsxAttribute','jsxNamespacedDeclaration','jsxNamespacedAttribute'],
 'json.ts':['jsonUse','jsonNestedUse'],
 'scope.ts':['privateDeclaration','privateUse','otherPrivateDeclaration'],
 'data.json':['jsonKey'],
};
function selection(file,marker){
 const source=fs.readFileSync(path.join(root,file),'utf8');
 const tag=file==='data.json'?`"${marker}"`:`/*#${marker}*/`;
 const found=source.indexOf(tag);
 assert.notEqual(found,-1,`${file}/${marker}`);
 return {file,...position(lineStarts(source),found+(file==='data.json'?0:tag.length))};
}
function at(file,marker,length){
 const start=selection(file,marker);
 return {file,start:{line:start.line,column:start.column},end:{line:start.line,column:start.column+length}};
}
const role=(marker,length=4)=>at('roles.ts',marker,length);
const scope=(marker)=>at('scope.ts',marker,6);
const group=(origins,occurrences)=>({origins,occurrences});
function expectedGroups(marker){
 const member=group([role('parameterName')],[role('parameterName'),role('constructorMember'),role('memberUse')]);
 const parameter=group([role('parameterName')],[role('parameterUse')]);
 const shapeLocations=['shapeKey','shorthand','shapeUse','bindingKey','bindingShort'].map(marker=>role(marker));
 const shape=group([role('shapeKey')],shapeLocations);
 const literal=group([role('literalDeclaration',5)],[role('literalDeclaration',5),role('literalUse',5),role('templateUse',5),role('dotUse',3)]);
 const jsonLocation=(nested=false)=>({file:'data.json',start:{line:1,column:nested?27:2},end:{line:1,column:nested?36:11}});
 switch(marker){
  case 'memberModifier': case 'memberUse': return [member,parameter];
  case 'parameterName': return [group(member.origins,member.occurrences.slice(0,2)),parameter];
  case 'shapeKey': case 'bindingKey': return [shape];
  case 'shorthand': return [shape,group([role('localName')],[role('localName')])];
  case 'bindingShort': return [group(shape.origins,shapeLocations.slice(0,-1)),group([role('bindingShort')],[role('bindingShort'),role('bindingShortUse')])];
  case 'literalDeclaration': case 'literalUse': return [literal];
  case 'computedUse': return [group([role('computedLocal',3)],[role('computedLocal',3),role('computedUse',3)])];
  case 'jsxDeclaration': case 'jsxNamespacedDeclaration': case 'jsxAttribute': {
   const value=at('view.tsx',marker,marker==='jsxNamespacedDeclaration'?8:7);
   return [group([value],[value])];
  }
  case 'jsonKey': case 'jsonUse': return [group([jsonLocation()],[jsonLocation(),at('json.ts','jsonUse',7),at('json.ts','jsonLiteralUse',9)])];
  case 'jsonNestedUse': return [group([jsonLocation(true)],[jsonLocation(true),at('json.ts','jsonNestedUse',7)])];
  case 'privateDeclaration': case 'privateUse': return [group([scope('privateDeclaration')],[scope('privateDeclaration'),scope('privateUse')])];
  case 'otherPrivateDeclaration': return [group([scope('otherPrivateDeclaration')],[scope('otherPrivateDeclaration'),scope('otherPrivateUse')])];
  case 'jsxNamespacedAttribute': return [];
  default: throw new Error(`Missing asserted native case ${marker}`);
 }
}

test('native member inventory roles, exact literal keys and search scope',async()=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']){
  for(const [file,names] of Object.entries(markers)) for(const marker of names){
   try {
    const answer=await compilerHookOracle(path.join(root,config),{operation:'member',at:selection(file,marker)});
    rows.push({config,file,marker,answer});
   } catch(error) {
    rows.push({config,file,marker,error:String(error)});
   }
   fs.writeFileSync('.cc/temp/member-inventory-native.json',JSON.stringify(rows,null,2));
  }
 }
 fs.writeFileSync('.cc/temp/member-inventory-native.json',JSON.stringify(rows,null,2));
 assert.equal(rows.length,60);
 for(const {file,marker,answer,error} of rows){
  assert.equal(error,undefined,`${file}/${marker}: ${error}`);
  assert.equal(answer.compiler,'7.0.2',`${file}/${marker}`);
  assert.ok(Array.isArray(answer.groups),`${file}/${marker}`);
  const expected=expectedGroups(marker);
  if(expected) assert.deepEqual(answer.groups.map(value=>group(value.origins.map(origin=>origin.location),value.occurrences)),expected,`${file}/${marker}`);
  if(file==='scope.ts'){
   const expectedLines=marker==='otherPrivateDeclaration'?[8,9]:[2,3];
   assert.deepEqual(answer.candidateObservations.filter(row=>row.inDomain).map(row=>row.location.start.line),expectedLines);
   assert.deepEqual(answer.candidateDemands.map(row=>row.start.line),expectedLines);
  }
 }
 // Raw native groups remain separate from product lexical-role and JSX-gap
 // expectations. The source-domain admission is the documented normalization.
});
