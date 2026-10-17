export {};
declare function assert(condition: unknown): asserts condition;
function andTrue(x: number, a: MissingAndFirst, b: MissingAndSecond) {
    if (x === /*@andFirst*/a && x === /*@andSecond*/b) return /*#andTrue*/x;
}
function orFalse(x: number, a: MissingOrFirst, b: MissingOrSecond) {
    if (!(x === /*@orFirst*/a || x === /*@orSecond*/b)) return /*#orFalse*/x;
}
function orTrue(x: number, a: MissingOrTrueFirst, b: MissingOrTrueSecond) {
    if (x === /*@orTrueFirst*/a || x === /*@orTrueSecond*/b) return /*#orTrue*/x;
}
function andFalse(x: number, a: MissingAndFalseFirst, b: MissingAndFalseSecond) {
    if (!(x === /*@andFalseFirst*/a && x === /*@andFalseSecond*/b)) return /*#andFalse*/x;
}
function nestedBoolean(x: number, a: MissingNestedFirst, b: MissingNestedSecond) {
    if (true === (x === /*@nestedFirst*/a && x === /*@nestedSecond*/b)) return /*#nestedBoolean*/x;
}
function assertionAnd(x: number, a: MissingAssertionFirst, b: MissingAssertionSecond) {
    assert(x === /*@assertionFirst*/a && x === /*@assertionSecond*/b);
    return /*#assertionAnd*/x;
}
function assertionOr(x: number, a: MissingAssertionOrFirst, b: MissingAssertionOrSecond) {
    assert(x === /*@assertionOrFirst*/a || x === /*@assertionOrSecond*/b);
    return /*#assertionOr*/x;
}
function switchCases(x: number, a: MissingCaseFirst, b: MissingCaseSecond) {
    switch (x) { case /*@caseFirst*/a: case /*@caseSecond*/b: return /*#switchCases*/x; }
}
function switchDefault(x: number, a: MissingDefaultFirst, b: MissingDefaultSecond) {
    switch (x) { case /*@defaultFirst*/a: break; case /*@defaultSecond*/b: break; default: return /*#switchDefault*/x; }
}
function healthyLogical(x: number, a: number, b: number) {
    if (x === a && x === b) return /*#healthyLogical*/x;
}
function healthyAssertion(x: number) { assert(x === x); return /*#healthyAssertion*/x; }
function healthySwitch(x: number, a: number, b: number) {
    switch (x) { case a: case b: return /*#healthySwitch*/x; }
}
function optionalSatisfied(x: { p: number } | undefined) {
    if ((x satisfies { p: number } | undefined)?.p) return /*#optionalSatisfied*/x;
}
function optionalAssigned(x: { p: number } | undefined, other: { p: number } | undefined) {
    if ((x = other)?.p) return /*#optionalAssigned*/x;
}
function optionalSatisfiedEquality(x: { p: number } | undefined) {
    if ((x satisfies { p: number } | undefined)?.p !== undefined) return /*#optionalSatisfiedEquality*/x;
}
function optionalAssignedEquality(x: { p: number } | undefined, other: { p: number } | undefined) {
    if ((x = other)?.p !== undefined) return /*#optionalAssignedEquality*/x;
}
