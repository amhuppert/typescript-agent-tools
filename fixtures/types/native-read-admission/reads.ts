import { importedCount as first, importedCount as second, importedUnion, importedFunction } from "./exports";
/*first-alias-read*/ first;
/*second-alias-read*/ second;
/*union-alias-read*/ importedUnion;
/*function-alias-read*/ importedFunction;
if (typeof second === "number") {
    /*other-alias-condition-read*/ first;
}
if (typeof importedUnion === "number") {
    /*same-alias-condition-read*/ importedUnion;
}
function importedBoundary(): void {
    /*captured-alias-read*/ first;
}
const importedArrow = (): number => {
    /*captured-alias-arrow-read*/ first;
    let local: number;
    /*local-arrow-nonnull-read*/ local!;
    return 0;
};
export const fixed = 1;
/*fixed-read*/ fixed;
export let count: number = 1;
/*count-read*/ count;
export declare let declared: string | number;
/*declared-read*/ declared;
export let refined: string | number = 1;
/*refined-read*/ refined;
let flag = false;
/*flag-read*/ flag;
let automatic;
/*automatic-read*/ automatic;
let unrelated: string | number = "text";
unrelated = 2;
/*after-unrelated-assignment*/ count;
count = 2;
/*after-own-assignment*/ count;
if (typeof unrelated === "string") {
    /*inside-unrelated-condition*/ count;
}
if (typeof refined === "number") {
    /*inside-own-condition*/ refined;
}
if (true) {
    /*inside-constant-condition*/ count;
}
if (false) {
    /*inside-unreachable-condition*/ refined;
}
while (false) {
    /*inside-unreachable-loop*/ count;
}
/*after-unreachable-loop*/ count;
function boundaries(): void {
    const local: number = 3;
    /*local-start-read*/ local;
    /*outer-function-read*/ count;
}
{
    let count: string = "inner";
    count = "updated";
    /*shadowed-inner-read*/ count;
}
/*after-shadowed-assignment*/ count;
{
    let count: string | number = 1;
    if (typeof count === "string") {}
}
/*after-shadowed-condition*/ count;
export declare let impossible: never;
if (typeof impossible === "string") {
    /*never-condition-read*/ impossible;
}
if (typeof Missing === "string") {
    /*unknown-condition-read*/ count;
}
export let assignedUnion: string | number = 0;
/*definite-union-write*/ assignedUnion = "value";
/*logical-union-write*/ assignedUnion ||= "value";
function compoundBoundary(): void {
    /*numeric-compound-write*/ count += 2;
}
/*constant-write*/ fixed = 2;
function uninitializedBoundary(): void {
    let notDefinite: number;
    /*uninitialized-number-read*/ notDefinite;
    /*nonnull-initialization-read*/ notDefinite!;
    (/*parenthesized-nonnull-read*/ notDefinite)!;
}
function assignmentKinds(): void {
    let numeric: number = 0;
    /*compound-plus*/ numeric += 1;
    /*compound-minus*/ numeric -= 1;
    /*compound-multiply*/ numeric *= 1;
    /*compound-power*/ numeric **= 1;
    /*compound-divide*/ numeric /= 1;
    /*compound-remainder*/ numeric %= 1;
    /*compound-and*/ numeric &= 1;
    /*compound-or*/ numeric |= 1;
    /*compound-xor*/ numeric ^= 1;
    /*compound-shift-left*/ numeric <<= 1;
    /*compound-shift-right*/ numeric >>= 1;
    /*compound-shift-unsigned*/ numeric >>>= 1;
    /*compound-postfix*/ numeric++;
    ++/*compound-prefix*/ numeric;
    /*compound-parenthesized*/ (numeric) += 1;
    /*compound-nonnull*/ numeric! += 1;
    let literal: 1 = 1;
    /*literal-definite*/ literal = 1;
    /*literal-compound*/ literal += 1;
    /*literal-compound-like*/ literal = literal + 1;
    /*literal-bitwise-rhs*/ literal = literal | 1;
    let literals: 1 | 2 = 1;
    /*literal-union-definite*/ literals = 2;
    /*literal-union-compound-like*/ literals = literals * 2;
    /*after-literal-compound-like*/ literals;
}
function compoundReadKinds(): void {
    let literal: 1 = 1;
    literal += 1;
    /*after-literal-compound*/ literal;
}
function ambientCompoundRead(): void {
    declared += 1;
    /*after-ambient-union-compound*/ declared;
}
function definiteKinds(): void {
    let mixed: string | number = 0;
    /*logical-and-write*/ mixed &&= "text";
    /*logical-nullish-write*/ mixed ??= "text";
    for (/*for-in-write*/ mixed in {}) {}
    for (/*for-of-write*/ mixed of [1]) {}
}

function ordinaryLoops(): void {
    const unchanged: number = 1;
    let other: number = 0;
    while (typeof other === "number") {
        /*inside-while-identity*/ unchanged;
        other += 1;
        break;
    }
    /*after-while-identity*/ unchanged;
    do {
        /*inside-do-identity*/ unchanged;
        other += 1;
    } while (false);
    /*after-do-identity*/ unchanged;
    for (other = 0; false; other += 1) {
        /*inside-for-unreachable*/ unchanged;
    }
    /*after-for-identity*/ unchanged;
    for (other in {}) {
        /*inside-for-in-identity*/ unchanged;
        break;
    }
    /*after-for-in-identity*/ unchanged;
    for (other of [1]) {
        /*inside-for-of-identity*/ unchanged;
        break;
    }
    /*after-for-of-identity*/ unchanged;
    while (true) {
        /*inside-cyclic-identity*/ unchanged;
        other += 1;
    }
    /*after-infinite-loop*/ unchanged;
}

function labelledLoops(): void {
    const unchanged: number = 1;
    outer: inner: for (;;) {
        for (;;) {
            /*before-labelled-break*/ unchanged;
            break outer;
        }
        /*after-labelled-break-unreachable*/ unchanged;
    }
    /*after-labelled-break*/ unchanged;
    next: while (true) {
        /*before-labelled-continue*/ unchanged;
        continue next;
        /*after-labelled-continue-unreachable*/ unchanged;
    }
    /*after-labelled-infinite-loop*/ unchanged;
}
function labelledBlock(): void {
    const unchanged: number = 1;
    block: {
        /*inside-labelled-block*/ unchanged;
        break block;
        /*after-block-break-unreachable*/ unchanged;
    }
    /*after-labelled-block*/ unchanged;
}
function escapedLabel(): void {
    const unchanged: number = 1;
    \u0065scaped: while (true) {
        /*before-escaped-label-break*/ unchanged;
        break escaped;
    }
    /*after-escaped-label*/ unchanged;
}
function labelBoundary(): void {
    const unchanged: number = 1;
    boundary: {
        const inner = (): number => {
            break boundary;
            /*invalid-label-target-read*/ unchanged;
            return 0;
        };
    }
}

declare function ordinaryEffect(): void;
declare function genericOrdinaryEffect<T>(value: T): void;
declare const annotatedEffect: () => void;
function ordinaryOverload(value: number): void;
function ordinaryOverload(value: string): void;
function ordinaryOverload(value: string | number): void { MissingBody; }
const untypedEffect = MissingInitializer;
function ordinaryCalls(): void {
    const unchanged: number = 1;
    ordinaryEffect();
    /*after-ordinary-call*/ unchanged;
    genericOrdinaryEffect(1);
    /*after-generic-ordinary-call*/ unchanged;
    annotatedEffect();
    /*after-annotated-call*/ unchanged;
    ordinaryOverload(MissingArgument);
    /*after-overload-ordinary-call*/ unchanged;
    untypedEffect();
    /*after-untyped-call*/ unchanged;
    MissingCall();
    /*after-unresolved-call*/ unchanged;
    (ordinaryEffect(), ordinaryEffect());
    /*after-comma-ordinary-calls*/ unchanged;
}
declare function effectAssert(value: unknown): asserts value;
declare function effectPredicate(value: unknown): value is string;
declare function effectNever(): never;
function excludedEffects(): void {
    const unchanged: number = 1;
    effectAssert(unchanged);
    /*after-asserting-call*/ unchanged;
    effectNever();
    /*after-never-call*/ unchanged;
}
function ordinaryPredicateEffect(): void {
    const unchanged: number = 1;
    effectPredicate(unchanged);
    /*after-nonasserting-predicate-call*/ unchanged;
}

function falseAssertionEffects(): void {
    let first: string | number = 1;
    effectAssert(false);
    first = "assigned";
    /*after-false-assertion-assignment*/ first;
}
function parenthesizedFalseAssertion(): void {
    let first: string | number = 1;
    effectAssert(((false)));
    first = "assigned";
    /*after-parenthesized-false-assertion*/ first;
}
function andFalseAssertionLeft(): void {
    let first: string | number = 1;
    effectAssert(false && MissingArgument);
    first = "assigned";
    /*after-and-left-false-assertion*/ first;
}
function andFalseAssertionRight(): void {
    let first: string | number = 1;
    effectAssert(MissingArgument && false);
    first = "assigned";
    /*after-and-right-false-assertion*/ first;
}
function orFalseAssertion(): void {
    let first: string | number = 1;
    effectAssert(false || (false && MissingArgument));
    first = "assigned";
    /*after-or-false-assertion*/ first;
}
function trueAssertionEffects(): void {
    let first: string | number = 1;
    effectAssert(true);
    first = "assigned";
    /*after-true-assertion-assignment*/ first;
}
function trueOrAssertion(): void {
    let first: string | number = 1;
    effectAssert(false || true);
    first = "assigned";
    /*after-true-or-assertion*/ first;
}

function logicalFlowIdentity(): void {
    const unchanged: number = 1;
    let other: number = 2;
    (typeof unchanged === "string") && MissingUse;
    /*after-pure-logical-and*/ unchanged;
    (typeof unchanged === "string") || MissingUse;
    /*after-pure-logical-or*/ unchanged;
    (typeof unchanged === "string") ?? MissingUse;
    /*after-pure-logical-nullish*/ unchanged;
    !((typeof unchanged === "string") && MissingUse);
    /*after-negated-pure-logical*/ unchanged;
    (typeof other === "string") && (other = 3);
    /*after-logical-and-effects*/ unchanged;
    (typeof other === "string") || (other = 4);
    /*after-logical-or-effects*/ unchanged;
    (typeof other === "string") ?? (other = 5);
    /*after-logical-nullish-effects*/ unchanged;
    if (((typeof other === "string") && true) || false) {
        /*inside-nested-logical-condition*/ unchanged;
    }
    if (!((typeof other === "string") && false)) {
        /*inside-negated-logical-condition*/ unchanged;
    }
}

function truthinessIdentity(): void {
    let value: number = 1;
    if (value) {
        /*inside-matching-number-truthy*/ value;
    } else {
        /*inside-matching-number-falsy*/ value;
    }
    effectAssert(value);
    /*after-matching-number-assertion*/ value;
    effectAssert(value || false);
    /*after-number-or-false-assertion*/ value;
    effectAssert(false || value);
    /*after-false-or-number-assertion*/ value;
    effectAssert(value && true);
    /*after-number-and-true-assertion*/ value;
    effectAssert(0);
    /*after-zero-assertion*/ value;
    effectAssert(null);
    /*after-null-assertion*/ value;
    effectAssert(!true);
    /*after-negated-true-assertion*/ value;
}
function literalTruthiness(): void {
    const value: true = true;
    if (value) {
        /*inside-matching-literal-true*/ value;
    } else {
        /*inside-matching-literal-false*/ value;
    }
}
function falseAssertionAntecedent(): void {
    let value: string | number = 1;
    effectAssert(false);
    /*false-assertion-consumed-union-antecedent*/ value;
}
function directFalseAssertion(): void {
    let value: number = 1;
    effectAssert(false);
    /*after-direct-false-assertion*/ value;
}
function directAndFalseAssertion(): void {
    let value: number = 1;
    effectAssert(false && false);
    /*after-direct-and-false-assertion*/ value;
}
function directOrFalseAssertion(): void {
    let value: number = 1;
    effectAssert(false || false);
    /*after-direct-or-false-assertion*/ value;
}
