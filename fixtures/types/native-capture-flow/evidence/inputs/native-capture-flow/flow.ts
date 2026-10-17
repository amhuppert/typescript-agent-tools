export function unchangedClosures(): void {
    const constant: number = 1;
    let mutable: number = 1;
    const arrow = () => /*constant-arrow*/ constant;
    const expression = function () { return /*constant-function*/ constant; };
    const mutableArrow = () => /*mutable-arrow*/ mutable;
    const mutableExpression = function () { return /*mutable-function*/ mutable; };
}
export function closureAssignments(): void {
    let value: string | number = 1;
    const before = () => /*before-last*/ value;
    value = "later";
    const after = () => /*after-last*/ value;
    const functionAfter = function () { return /*function-after-last*/ value; };
    function declarationAfter() { return /*declaration-after-last*/ value; }
}
export function constantRefinement(): void {
    const value: string | number = 1;
    const arrow = () => /*constant-refined-arrow*/ value;
    const expression = function () { return /*constant-refined-expression*/ value; };
    function declaration() { return /*constant-refined-declaration*/ value; }
}
export function compoundPosition(condition: boolean): void {
    let value: string | number = 1;
    if (condition) {
        value = "later";
        const during = () => /*compound-inside*/ value;
    }
    const after = () => /*compound-after*/ value;
}
export function nestedAssignment(): void {
    let value: string | number = 1;
    function mutate() { value = "later"; }
    const after = () => /*nested-assignment*/ value;
}
export function uninitialized(): void {
    let value: number;
    const before = () => /*never-initialized*/ value;
}
export function definiteAssignment(): void {
    let value: number;
    const before = () => /*definitely-assigned-before*/ value;
    value = 1;
    const after = () => /*definitely-assigned-after*/ value;
}
export function compoundAssignment(): void {
    let value: number;
    value += 1;
    const after = () => /*compound-not-definite*/ value;
}
export function methodKinds(): void {
    const value: string | number = 1;
    const object = {
        method() { return /*object-method*/ value; },
        get accessor() { return /*object-getter*/ value; },
        set accessor(input: number) { /*object-setter*/ value; },
    };
    const expression = class {
        method() { return /*class-expression-method*/ value; }
        get accessor() { return /*class-expression-getter*/ value; }
        set accessor(input: number) { /*class-expression-setter*/ value; }
    };
    class Declaration {
        method() { return /*class-declaration-method*/ value; }
        get accessor() { return /*class-declaration-getter*/ value; }
        set accessor(input: number) { /*class-declaration-setter*/ value; }
    }
}
export let exported: string | number = 1;
export const exportedArrow = () => /*exported-let*/ exported;
let local: string | number = 1;
const localArrow = () => /*module-local-let*/ local;
export { local };
let typeExported: string | number = 1;
const typeExportedArrow = () => /*type-only-export*/ typeExported;
export type { typeExported };
export function parameterCapture(value: string | number): void {
    const before = () => /*parameter-before-last*/ value;
    value = "later";
    const after = () => /*parameter-after-last*/ value;
}
export function noAssignmentParameter(value: number): void {
    const after = () => /*parameter-no-assignment*/ value;
}
