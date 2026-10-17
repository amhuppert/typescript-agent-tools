import { exportedCondition as importedCondition, importedKind, importedValue } from "./imported";
function unchanged(x: number) { const a = x === x; if (a) return /*#unchanged*/x; }
function narrowed(x: number | string) { const a = typeof x === "number"; if (a) return /*#narrowed*/x; }
function falseArm(x: number | string) { const a = typeof x === "number"; if (!a) return /*#falseArm*/x; }
function annotated(x: number | string) { const a: boolean = typeof x === "number"; if (a) return /*#annotated*/x; }
function mutableAlias(x: number | string) { let a = typeof x === "number"; if (a) return /*#mutableAlias*/x; }
function uninitialized(x: number | string) { const a: boolean; if (a) return /*#uninitialized*/x; }
function assignedParameter(x: number | string) { const a = typeof x === "number"; if (a) { x = x; return /*#assignedParameter*/x; } }
function unusedAlias(x: number | string, y: boolean) { const a = y; if (a) return /*#unusedAlias*/x; }
function constReference() { const x: number | string = crossValue; const a = typeof x === "number"; if (a) return /*#constReference*/x; }
function mutableReference() { let x: number | string = crossValue; const a = typeof x === "number"; if (a) return /*#mutableReference*/x; }
function assignedReference() { let x: number | string = crossValue; const a = typeof x === "number"; if (a) { x = crossValue; return /*#assignedReference*/x; } }
function readonlyReference(x: {readonly value: number | string}) { const a = typeof x.value === "number"; if (a) return x./*#readonlyReference*/value; }
function mutableProperty(x: {value: number | string}) { const a = typeof x.value === "number"; if (a) return x./*#mutableProperty*/value; }
function parenthesized(x: number | string) { const a = (typeof x === "number"); if (a) return /*#parenthesized*/x; }
function satisfied(x: number | string) { const a = (typeof x === "number") satisfies boolean; if (a) return /*#satisfied*/x; }
function negated(x: number | string) { const a = !(typeof x === "number"); if (a) return /*#negated*/x; }
function joined(x: number | string) { const a = typeof x === "number"; if (a) {} return /*#joined*/x; }
function recovery(x: number, a: MissingLocal) { const b = x === /*@localRecovery*/a; if (b) return /*#recovery*/x; }
function hiddenRecovery(x: number, a: MissingHidden) { const b: boolean = x === /*@hiddenRecovery*/a; if (b) return /*#hiddenRecovery*/x; }
function destructured(xy: {x: number | string, y: boolean}) { const {x, y} = xy; const a = typeof x === "number"; if (a) return /*#destructured*/x; }
function crossNarrowed() { if (crossCondition) return /*#crossNarrowed*/crossValue; }
function crossUnchanged() { if (crossIrrelevant) return /*#crossUnchanged*/crossValue; }
function crossFailed() { if (crossRecovery) return /*#crossFailed*/crossValue; }
function depth4(x: number | string) { const a1 = typeof x === "number"; const a2 = a1; const a3 = a2; const a4 = a3; if (a4) return /*#depth4*/x; }
function depth5(x: number | string) { const a1 = typeof x === "number"; const a2 = a1; const a3 = a2; const a4 = a3; const a5 = a4; if (a5) return /*#depth5*/x; }
function depth6(x: number | string) { const a1 = typeof x === "number"; const a2 = a1; const a3 = a2; const a4 = a3; const a5 = a4; const a6 = a5; if (a6) return /*#depth6*/x; }
function importedAlias() { if (importedCondition) return /*#importedAlias*/crossValue; }
function siblingUnwind(x: number, a: MissingSiblingFirst, b: MissingSiblingSecond) {
    const a1 = x === /*@siblingFirst*/a; const a2 = a1; const a3 = a2; const a4 = a3;
    const right = x === /*@siblingSecond*/b; const both = a4 && right;
    if (both) return /*#siblingUnwind*/x;
}
function crossProperty() { if (crossDiscriminant) return /*#crossProperty*/crossObject; }
function crossPredicateCall() { if (crossCall) return /*#crossPredicateCall*/crossValue; }
function crossUnrelatedCall() { if (crossCallUnrelated) return /*#crossUnrelatedCall*/crossValue; }
function crossDestructuring() { if (importedKind === "a") return /*#crossDestructuring*/importedValue; }
function recoveryDepth5(x: number, a: MissingDepth5) { const a1 = x === /*@depth5*/a; const a2 = a1; const a3 = a2; const a4 = a3; const a5 = a4; if (a5) return /*#recoveryDepth5*/x; }
function recoveryDepth6(x: number, a: MissingDepth6) { const a1 = x === /*@depth6*/a; const a2 = a1; const a3 = a2; const a4 = a3; const a5 = a4; const a6 = a5; if (a6) return /*#recoveryDepth6*/x; }
