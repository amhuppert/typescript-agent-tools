import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/native-member-values/',import.meta.url));
async function fresh(config,action,file='cases.ts'){
  const native=await loadSemanticApi();
  assert.equal(native.version,'7.0.2');
  const configPath=path.join(root,config);
  const api=new native.API({tsserverPath:native.binary,cwd:root});let snapshot;
  try{
    snapshot=api.updateSnapshot({openProject:configPath});
    const project=snapshot.getProject(configPath);
    const diagnosticCodes=[...project.program.getConfigFileParsingDiagnostics(),
      ...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics(),
      ...project.program.getBindDiagnostics()].map(d=>d.code);
    // This accepted tool option state deliberately retains native TS5052 as
    // diagnostic-only policy. No other setup diagnostic is dismissed.
    assert.deepEqual(diagnosticCodes,config==='loose-exact.json'?[5052]:[]);
    const source=project.program.getSourceFile(path.join(root,file));
    await action(native,project.checker,source);
  }finally{try{snapshot?.dispose();}finally{api.close();}}
}
const own=(checker,source,name)=>{
  const declaration=source.statements[0].members.find(member=>member.name?.text===name);
  assert.ok(declaration,name);
  return{declaration,symbol:checker.getSymbolAtLocation(declaration.name)};
};
const variable=(checker,source,name)=>{
  const declaration=source.statements.flatMap(statement=>statement.declarationList?.declarations??[])
    .find(declaration=>declaration.name.text===name);
  assert.ok(declaration,name);
  return checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
};

test('merged property and accessor flags keep separate native read and write precedence',async()=>{
  await fresh('merged-flags.json',(native,checker,source)=>{
    for(const name of ['mixed','genericMixed']){
      const type=variable(checker,source,name);
      const property=checker.getPropertiesOfType(type).find(symbol=>symbol.name==='value');
      assert.ok(property.flags&native.SymbolFlags.Property,name);
      assert.ok(property.flags&native.SymbolFlags.GetAccessor,name);
      assert.ok(property.flags&native.SymbolFlags.SetAccessor,name);
      const read=checker.getTypeOfSymbol(property);
      assert.equal(read.isErrorType(),false,name);
      assert.equal(checker.typeToString(read),'string',name);
      const statement=source.statements.find(statement=>statement.expression?.left?.expression?.text===name);
      assert.ok(statement,name);
      const write=checker.getTypeAtLocation(statement.expression.left);
      assert.equal(write.isErrorType(),false,name);
      assert.equal(checker.typeToString(write),'string',name);
    }
  },'merged-flags.ts');
});

test('native accessor raw read and own setter location ask distinct questions',async()=>{
  await fresh('tsconfig.json',(native,checker,source)=>{
    for(const[name,summary]of [['getterOnly','string'],['setterOnly','number'],['paired','string'],
      ['fromSetter','boolean'],['ignoredBadSetter','string']]){
      const{symbol}=own(checker,source,name);
      const read=checker.getTypeOfSymbol(symbol);
      assert.equal(read.isErrorType(),false,name);
      assert.equal(checker.typeToString(read),summary,name);
      const getter=source.statements[0].members.find(declaration=>
        declaration.kind===native.SyntaxKind.GetAccessor&&declaration.name?.text===name);
      if(getter) assert.equal(checker.typeToString(checker.getTypeOfSymbolAtLocation(symbol,getter.name)),summary);
    }
    const{symbol}=own(checker,source,'paired');
    const setter=source.statements[0].members.find(declaration=>
      declaration.kind===native.SyntaxKind.SetAccessor&&declaration.name?.text==='paired');
    const atSetter=checker.getTypeOfSymbolAtLocation(symbol,setter.name);
    assert.equal(atSetter.flags,native.TypeFlags.Number);
    assert.equal(atSetter.isErrorType(),false);
    assert.equal(checker.typeToString(atSetter),'number');
    // Product Q4 deliberately asks the raw read at either declaration anchor.
    assert.equal(checker.typeToString(checker.getTypeOfSymbol(symbol)),'string');
  });
});

test('native setter value parameter distinguishes zero one and exactly-two-this roles',async()=>{
  await fresh('setter-parameters.json',(native,checker,source)=>{
    for(const[name,summary,parameters]of [
      ['none','any',[]],['one','string',['value']],['two','number',['this','value']],
      ['twoOrdinary','boolean',['first','second']],['three','Setters',['this','value','extra']],
    ]){
      const{declaration,symbol}=own(checker,source,name);
      assert.equal(declaration.kind,native.SyntaxKind.SetAccessor);
      assert.deepEqual(declaration.parameters.map(parameter=>parameter.name.text),parameters);
      const raw=checker.getTypeOfSymbol(symbol);
      assert.equal(raw.isErrorType(),false,name);
      assert.equal(checker.typeToString(raw),summary,name);
    }
  },'setter-parameters.ts');
});

test('native optional property and method raw reads retain all strict/exact option states',async()=>{
  for(const config of ['tsconfig.json','exact.json','loose.json','loose-exact.json']){
    await fresh(config,(native,checker,source)=>{
      const strict=config==='tsconfig.json'||config==='exact.json';
      const exact=config==='exact.json';
      for(const[name,base]of [['optional','string'],['optionalMethod','(value: number) => string']]){
        const{symbol,declaration}=own(checker,source,name);
        assert.ok(symbol.flags&native.SymbolFlags.Optional,name);
        const raw=checker.getTypeOfSymbol(symbol);
        assert.equal(raw.isErrorType(),false,name);
        assert.equal(raw.flags,strict?native.TypeFlags.Union:
          name==='optional'?native.TypeFlags.String:native.TypeFlags.Object,name);
        assert.equal(checker.typeToString(raw),strict?
          name==='optional'?'string | undefined':'((value: number) => string) | undefined':base,name);
        const located=checker.getTypeOfSymbolAtLocation(symbol,declaration.name);
        assert.equal(located.isErrorType(),false,name);
        assert.equal(checker.typeToString(located),exact?base:checker.typeToString(raw),name);
        if(strict){
          assert.equal(raw.getTypes().length,2);
          assert.equal(raw.getTypes()[0].flags,native.TypeFlags.Undefined);
          const ordinaryUndefined=checker.getUndefinedType();
          assert.equal(raw.getTypes()[0].id===ordinaryUndefined.id,!exact);
        }
      }
    });
  }
});

test('native method overloads and instantiated method signatures preserve canonical type dependencies',async()=>{
  await fresh('tsconfig.json',(native,checker,source)=>{
    const{symbol}=own(checker,source,'overloaded');
    assert.equal(symbol.declarations.length,2);
    assert.ok(symbol.declarations.every(declaration=>declaration.kind===native.SyntaxKind.MethodSignature));
    const overloaded=checker.getTypeOfSymbol(symbol);
    assert.equal(overloaded.flags,native.TypeFlags.Object);
    assert.equal(overloaded.isErrorType(),false);
    assert.equal(checker.typeToString(overloaded),'{ (value: string): string; (value: number): number; }');
    assert.equal(checker.getSignaturesOfType(overloaded,native.SignatureKind.Call).length,2);
    const generic=variable(checker,source,'generic');
    assert.equal(generic.isErrorType(),false);
    assert.equal(checker.typeToString(generic),'Generic<string>');
    const method=checker.getPropertiesOfType(generic).find(symbol=>symbol.name==='method');
    const type=checker.getTypeOfSymbol(method);
    assert.equal(type.flags,native.TypeFlags.Object);
    assert.equal(type.isErrorType(),false);
    assert.equal(checker.typeToString(type),'(input: string) => string');
    const signature=checker.getSignaturesOfType(type,native.SignatureKind.Call)[0];
    const parameter=checker.getTypeOfSymbol(signature.getParameters()[0]);
    const returned=checker.getReturnTypeOfSignature(signature);
    assert.equal(parameter.id,returned.id);
    assert.equal(returned.flags,native.TypeFlags.String);
  });
});

test('native class instance inventory includes private/protected members and excludes static exports',async()=>{
  await fresh('tsconfig.json',(native,checker,source)=>{
    const instance=variable(checker,source,'declared');
    assert.equal(instance.flags,native.TypeFlags.Object);
    assert.equal(instance.isErrorType(),false);
    const properties=checker.getPropertiesOfType(instance);
    assert.deepEqual(properties.map(property=>property.name),['protectedValue','hiddenValue','paired','method']);
    assert.equal(checker.typeToString(checker.getTypeOfSymbol(properties[0])),'number');
    assert.equal(checker.typeToString(checker.getTypeOfSymbol(properties[1])),'string');
  });
});

test('native annotation recovery stays distinct from genuine any and unknown accessor reads',async()=>{
  await fresh('recovery.json',(native,checker,source)=>{
    for(const[name,summary,flags,error]of [['genuineAny','any',native.TypeFlags.Any,false],
      ['genuineUnknown','unknown',native.TypeFlags.Unknown,false],
      ['ignoredBadSetter','string',native.TypeFlags.String,false],
      ['badRead','MissingRead',native.TypeFlags.Any,true]]){
      const{symbol}=own(checker,source,name);
      const type=checker.getTypeOfSymbol(symbol);
      assert.equal(type.flags,flags,name);
      assert.equal(type.isErrorType(),error,name);
      assert.equal(checker.typeToString(type),summary,name);
    }
  },'recovery.ts');
});

test('archived private native questions preserve missing/write identity and accumulated diagnostic phases',()=>{
  const evidence=path.join(root,'evidence');
  const digest=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
  const provenance=JSON.parse(fs.readFileSync(path.join(evidence,'provenance.json'),'utf8'));
  const answerBytes=fs.readFileSync(path.join(evidence,'answers.json'));
  assert.equal(digest(answerBytes),provenance.answerSha256);
  for(const input of [...provenance.capturedInputs,...provenance.code])
    assert.equal(digest(fs.readFileSync(path.join(evidence,input.archive))),input.sha256,input.archive);
  const rows=JSON.parse(answerBytes);
  assert.equal(rows.length,24);
  const lookup=(config,memberName)=>rows.find(row=>row.config===config&&row.variable==='plain'&&row.memberName===memberName).answer;
  const optional=lookup('exact.json','optional').memberValue;
  assert.equal(optional.read.containsMissing,true);
  assert.notEqual(optional.read.id,optional.write.id);
  assert.equal(optional.write.summary,'string');
  const method=lookup('exact.json','optionalMethod').memberValue;
  assert.equal(method.read.containsMissing,true);
  assert.equal(method.read.id,method.write.id);
  assert.equal(method.nonMissingRead.containsMissing,false);
  const badSetter=lookup('tsconfig.json','ignoredBadSetter').memberValue;
  assert.equal(badSetter.read.errorType,false);
  assert.equal(badSetter.write.errorType,true);
  assert.deepEqual(badSetter.afterReadDiagnosticCodes.filter(code=>code!==2318),[]);
  assert.deepEqual(badSetter.afterWriteDiagnosticCodes.filter(code=>code!==2318),[2304]);
  const badRead=lookup('tsconfig.json','lazyRead').memberValue;
  assert.equal(badRead.read.errorType,true);
  assert.equal(badRead.write.errorType,false);
  assert.deepEqual(badRead.afterReadDiagnosticCodes.filter(code=>code!==2318),[2304]);
  const looseExact=lookup('loose-exact.json','optional');
  assert.deepEqual(looseExact.observedPreflightDiagnostics.map(diagnostic=>diagnostic.code),[5052]);
  assert.deepEqual(looseExact.nativeProgramDiagnosticCodes,[5052]);
});
