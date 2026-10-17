import assert from 'node:assert/strict';
import {test} from 'node:test';
import {assertMemberJourney} from './native-corpus-comparison.mjs';
const loc=n=>({file:'cases.ts',start:{line:n,column:1},end:{line:n,column:8}});
const native={candidateLedger:[{location:loc(1)},{location:loc(2)}],nativeRangeGroups:[{origins:[{location:loc(1)}],occurrences:[loc(1),loc(2)]}]};
const answer=(reason)=>({outcome:reason?'partial':'complete',groups:[{declarations:[{location:loc(1)}],occurrences:(reason?[1]:[1,2]).map(n=>({location:loc(n)}))}],coverage:{gaps:reason?[{reason,location:loc(2)}]:[]}});
test('complete ranges and explicitly excluded flow/JSX Partial results obey CLI status',()=>{
 assert.doesNotThrow(()=>assertMemberJourney(answer(),native,0));
 for(const reason of ['flow_required','jsx_typing_required']) assert.doesNotThrow(()=>assertMemberJourney(answer(reason),native,1));
});
test('a failure gap, wrong exit code, missing range or extra empty group cannot pass',()=>{
 assert.throws(()=>assertMemberJourney(answer('instantiation_limit'),native,1));
 const poisoned=answer('flow_required');poisoned.coverage.gaps[0].causes=[{reason:'instantiation_limit'}];assert.throws(()=>assertMemberJourney(poisoned,native,1));
 assert.throws(()=>assertMemberJourney(answer('flow_required'),native,0));
 const missing=answer();missing.groups[0].occurrences.pop();assert.throws(()=>assertMemberJourney(missing,native,0));
 const extra=answer();extra.groups.push({declarations:[{location:loc(3)}],occurrences:[]});assert.throws(()=>assertMemberJourney(extra,native,0));
 const origin=answer();origin.groups[0].declarations[0].location=loc(3);assert.throws(()=>assertMemberJourney(origin,native,0));
});

test('omitted causes and non-candidate gaps cannot hide discrepancies',()=>{
 const omitted=answer('flow_required');omitted.coverage.gaps[0].omittedCauses=1;assert.throws(()=>assertMemberJourney(omitted,native,1));
 const invented=answer('flow_required');invented.coverage.gaps[0].location=loc(3);assert.throws(()=>assertMemberJourney(invented,native,1));
});

test('whole-query failures retain their product reason in the corpus verdict',()=>{
 const failed={outcome:'error',error:{kind:'SemanticDeadline',reason:'semantic_deadline'}};
 assert.throws(()=>assertMemberJourney(failed,native,1), /member query failed: SemanticDeadline \(semantic_deadline\)/);
});
