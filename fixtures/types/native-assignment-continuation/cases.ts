export {};
declare const healthy: number | string;
declare const propertyInput: { value: number | string };
declare const propertyBroken: { value: MissingPropertyProjection };
declare const indexBroken: { [name: string]: MissingIndexProjection };
declare const tupleInput: [number | string];
declare const tupleBroken: [MissingTupleProjection | undefined];
declare const nestedBroken: { item: [MissingNestedProjection | undefined] };
declare const emptyInput: {};
declare const badKey: {};
declare const later: { value: MissingLaterDefault };
declare const anyInput: any;
declare const neverInput: never;

function propertyHealthy() {
    let value: number | string;
    ({value} = propertyInput);
    /*#propertyHealthy*/value;
}
function propertyRecovery() {
    let value: number | string;
    ({value: value = later.value} = propertyBroken);
    /*#propertyRecovery*/value;
}
function indexRecovery() {
    let value: number | string;
    ({item: value = later.value} = indexBroken);
    /*#indexRecovery*/value;
}
function absentRecovery() {
    let value: number | string;
    ({absent: value = later.value} = emptyInput);
    /*#absentRecovery*/value;
}
function unusableKeyRecovery() {
    let value: number | string;
    ({[badKey]: value = later.value} = propertyInput);
    /*#unusableKeyRecovery*/value;
}
function shorthandRecovery() {
    let value: number | string;
    ({value = later.value} = propertyBroken);
    /*#shorthandRecovery*/value;
}
function tupleHealthy() {
    let value: number | string;
    [value] = tupleInput;
    /*#tupleHealthy*/value;
}
function tupleRecovery() {
    let value: number | string;
    [value = later.value] = tupleBroken;
    /*#tupleRecovery*/value;
}
function nestedRecovery() {
    let value: number | string;
    ({item: [value = later.value]} = nestedBroken);
    /*#nestedRecovery*/value;
}
function tupleDefaultHealthy() {
    let value: number | string;
    [value = healthy] = tupleInput;
    /*#tupleDefaultHealthy*/value;
}
function tupleOutOfBounds() {
    let value: number | string;
    [, value = later.value] = tupleInput;
    /*#tupleOutOfBounds*/value;
}
declare const tupleTail: [boolean, ...(number | string)[]];
function tupleRestElement() {
    let value: number | string;
    [, value] = tupleTail;
    /*#tupleRestElement*/value;
}
declare const arrayLike: { 0: number | string; length: 1 };
function arrayLikeProperty() {
    let value: number | string;
    [value] = arrayLike;
    /*#arrayLikeProperty*/value;
}
declare const iterBroken: Iterable<MissingIterationProjection>;
function iterationRecovery() {
    let value: number | string;
    [value = later.value] = iterBroken;
    /*#iterationRecovery*/value;
}
function actualAny() {
    let value: number | string;
    value = anyInput;
    /*#actualAny*/value;
}
function assignedNever() {
    let value: number | string;
    value = neverInput;
    /*#assignedNever*/value;
}
function assignedIdentity() {
    let value: number | string;
    value = healthy;
    /*#assignedIdentity*/value;
}
function incompatibleAssignment() {
    let value: number | string;
    value = true;
    /*#incompatibleAssignment*/value;
}
function freshBoolean() {
    let value: boolean;
    value = true;
    /*#freshBoolean*/value;
}
function regularBoolean(input: true) {
    let value: boolean;
    value = input;
    /*#regularBoolean*/value;
}
function scalarSkipsAssignment() {
    let value: number;
    value = later.value;
    /*#scalarSkipsAssignment*/value;
}
function forInAssignment() {
    let value: number | string;
    for (value in emptyInput) { /*#forInAssignment*/value; }
}
declare const iterable: Iterable<number | string>;
function forOfAssignment() {
    let value: number | string;
    for (value of iterable) { /*#forOfAssignment*/value; }
}
function bindingInitialHealthy() {
    let [value]: [number | string] = tupleInput;
    /*#bindingInitialHealthy*/value;
}
function bindingInitialRecovery() {
    let [value = later.value]: [number | string] = tupleBroken;
    /*#bindingInitialRecovery*/value;
}
type Left = { left: MissingLeftComparison };
type Right = { right: MissingRightComparison };
declare const objectInput: { left: number; right: string };
function reductionCold() {
    let value: Left | Right = objectInput;
    /*#reductionCold*/value;
}
function reductionWarm() {
    let first: Left | Right = objectInput;
    let second: Left | Right = objectInput;
    const /*#reductionWarm*/pair = (first, second);
}
function reductionWarmReverse() {
    let first: Left | Right = objectInput;
    let second: Left | Right = objectInput;
    const /*#reductionWarmReverse*/pair = (second, first);
}
type HealthyLeft = { left: number };
type HealthyRight = { right: string };
function healthyReductionWarm() {
    let first: HealthyLeft | HealthyRight = objectInput;
    let second: HealthyLeft | HealthyRight = objectInput;
    const /*#healthyReductionWarm*/pair = (first, second);
}
