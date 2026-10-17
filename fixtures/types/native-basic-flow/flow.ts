declare const stable: number;
declare let other: number;
declare let input: any;
export declare let selected: string | number;
export declare let guarded: number | "ready";
declare function bad(): Missing;
declare let rest: any;

export function parameterDefault(value: any = (other = 1)): void {
    /*parameter-default*/ stable;
}
export function defaultBranchOrder(value: any = (() => { bad(); return 0; })()): void {
    /*default-first-entry*/ stable;
}
export function patternDefault({ name: { nested = (other = 2) } = (other = 3) }: any = (other = 1)): void {
    /*pattern-default*/ stable;
}
export function bindingDefault(): void {
    const { name: { nested = (other = 2) } = (other = 3) } = input;
    /*binding-default*/ stable;
}
export function deleteProperty(): void {
    delete input.name;
    /*delete-property*/ stable;
    delete input["name"];
    /*delete-element*/ stable;
    delete (input.name);
    /*delete-parentheses*/ stable;
    delete input?.name;
    /*delete-optional*/ stable;
}
export function destructuringAssignment(): void {
    ([other] = input);
    /*array-assignment*/ stable;
    ({ item: other } = input);
    /*object-assignment*/ stable;
    ([other = 1] = input);
    /*array-default-assignment*/ stable;
    ({ item: other = 1 } = input);
    /*object-default-assignment*/ stable;
    ([{ item: [other] }] = input);
    /*nested-assignment*/ stable;
    ([, other, ...rest] = input);
    /*array-rest-assignment*/ stable;
    ({ other, ...rest } = input);
    /*object-rest-assignment*/ stable;
    ({ other = 1 } = input);
    /*shorthand-default-assignment*/ stable;
}
export function nestedPatternOrder(): void {
    ([{ [/*nested-pattern-key*/ guarded]: [other] } = (guarded = 1)] = input);
}
export function ordinaryPatternOrder(): void {
    ([{ [/*ordinary-pattern-key*/ guarded]: other }] = (guarded = 1));
}
export function matchingArrayAssignment(): void {
    ([selected] = input);
    /*matching-array-assignment*/ selected;
}
export function matchingObjectAssignment(): void {
    ({ item: selected } = input);
    /*matching-object-assignment*/ selected;
}
export function matchingSpreadAssignment(): void {
    ([...selected] = input);
    /*matching-spread-assignment*/ selected;
}
export function arrayMutationCall(): void {
    !guarded ? input.push(1) : 0;
    /*push-effect*/ guarded;
}
export function arrayMutationUnshift(): void {
    !guarded ? input.unshift(1) : 0;
    /*unshift-effect*/ guarded;
}
export function arrayMutationEscaped(): void {
    !guarded ? input.p\u0075sh(1) : 0;
    /*escaped-push-effect*/ guarded;
}
export function elementMutation(): void {
    !guarded ? (input[0] = 1) : 0;
    /*element-effect*/ guarded;
}
export function firstDeclaredBranch(): void {
    guarded ? input.push(1) : 0;
    /*first-declared-branch*/ guarded;
}
export function pureOtherMethod(): void {
    selected ? input.pop() : 0;
    /*pop-pure*/ selected;
}
export function pureElementCall(): void {
    selected ? input["push"](1) : 0;
    /*element-call-pure*/ selected;
}
