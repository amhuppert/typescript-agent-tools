export {};
type Assertion = <T>(value: T) => asserts value;
declare function declared<T>(value: T): asserts value;
const annotated: Assertion = declared;
const inferred = <T>(value: T): asserts value => {};
declare const typedHolder: { check: Assertion };
const inferredHolder = { check: inferred };
type Copy<T> = { [P in keyof T]: T[P] };
declare const mappedHolder: Copy<typeof inferredHolder>;
declare const optionalFunction: Assertion | undefined;
const optionalHolder = undefined as { check: Assertion } | undefined;
declare function getAssertion(): Assertion;
declare function predicate<T>(value: T): value is T;
declare function ordinary<T>(value: T): void;
const quick = (value: unknown): asserts value => {};
declare function brokenPredicate<T>(value: T): asserts value is T & MissingAssertedType;
declare const assertions: Assertion[];
const inferredAssertions = assertions;

function declarationTarget(value: unknown) { declared(value/*#declarationTarget*/); }
function annotatedTarget(value: unknown) { annotated(value/*#annotatedTarget*/); }
function inferredTarget(value: unknown) { inferred(value/*#inferredTarget*/); }
function parenthesizedTarget(value: unknown) { ((inferred))(value/*#parenthesizedTarget*/); }
function typedProperty(value: unknown) { typedHolder.check(value/*#typedProperty*/); }
function inferredReceiver(value: unknown) { inferredHolder.check(value/*#inferredReceiver*/); }
function mappedOrigin(value: unknown) { mappedHolder.check(value/*#mappedOrigin*/); }

class Assertions {
    check<T>(value: T): asserts value {}
    inferredField = <T>(value: T): asserts value => {};
    #privateField = <T>(value: T): asserts value => {};
    static check<T>(value: T): asserts value {}
    methodThis(value: unknown) { this.check(value/*#methodThis*/); }
    inferredClassField(value: unknown) { this.inferredField(value/*#inferredClassField*/); }
    privateField(value: unknown) { this.#privateField(value/*#privateField*/); }
}
class DerivedAssertions extends Assertions {
    methodSuper(value: unknown) { super.check(value/*#methodSuper*/); }
}
function staticMethod(value: unknown) { Assertions.check(value/*#staticMethod*/); }
namespace Checks { export function check<T>(value: T): asserts value {} }
function namespaceTarget(value: unknown) { Checks.check(value/*#namespaceTarget*/); }

function elementTarget(value: unknown) { typedHolder["check"](value/*#elementTarget*/); }
function nonNullTarget(value: unknown) { inferred!(value/*#nonNullTarget*/); }
function callTarget(value: unknown) { getAssertion()(value/*#callTarget*/); }
function optionalOwn(value: unknown) { optionalFunction?.(value/*#optionalOwn*/); }
function optionalReceiver(value: unknown) { optionalHolder?.check(value/*#optionalReceiver*/); }
function resultInitializer(value: unknown) { const result = inferred(value/*#resultInitializer*/); return result; }
function returnCall(value: unknown) { return inferred(value/*#returnCall*/); }
function booleanPredicate(value: unknown) { predicate(value/*#booleanPredicate*/); }
function ordinaryVoid(value: unknown) { ordinary(value/*#ordinaryVoid*/); }
function quickControl(value: unknown) { quick(value/*#quickControl*/); }
function recoveredPredicate(value: unknown) { brokenPredicate(value/*#recoveredPredicate*/); }
function recoveredNotStatement(value: unknown) { return brokenPredicate(value/*#recoveredNotStatement*/); }
function recoveredArgument(value: unknown) { inferred(missingAssertionArgument/*#recoveredArgument*/); }
function forOfExplicit(value: unknown) {
    for (const check of assertions) { check(value/*#forOfExplicit*/); }
}
function forOfInferred(value: unknown) {
    for (const check of inferredAssertions) { check(value/*#forOfInferred*/); }
}
