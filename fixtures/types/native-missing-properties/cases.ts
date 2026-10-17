export {};
declare const plain: { present: number };
plain./*#plainMissing*/absent;
plain./*#plainSpelling*/presnet;
plain./*#plainExisting*/present;
declare const bad: { hidden: MissingAnonymous };
bad./*#anonymousDisplay*/absent;
interface Named { hidden: MissingNamed; }
declare const named: Named;
named./*#namedDisplay*/absent;
declare const union: { present: number } | { hidden: MissingUnion };
union./*#unionMissing*/present;
declare const indexedUnion: { [key: string]: number } | { hidden: MissingIndexUnion };
indexedUnion./*#indexedUnion*/absent;
class Static { static present: MissingStatic; hidden: MissingInstance; }
declare const instance: Static;
instance./*#staticSuggestion*/present;
instance./*#staticSpellingMiss*/presnet;
class StaticInherited extends Static {}
declare const inherited: StaticInherited;
inherited./*#inheritedStaticSuggestion*/present;
declare const promise: Promise<{ present: number; hidden: MissingPromisePayload }>;
promise./*#promiseSuggestion*/present;
promise./*#promiseMissing*/absent;
declare const thenable: { then(callback: (value: { present: number }) => unknown): unknown };
thenable./*#thenableSuggestion*/present;
declare const badThenable: { then(callback: (value: MissingThenableValue) => unknown): unknown };
badThenable./*#thenableRecovery*/absent;
declare const numbers: number[];
numbers./*#libraryArray*/includes;
numbers./*#libraryArrayLate*/toSorted;
numbers./*#libraryCase*/Includes;
declare const text: string;
text./*#libraryString*/replaceAll;
declare const date: Date;
date./*#libraryDate*/toTemporalInstant;
class Spelling {
    private secret: number;
    protected protectedValue: number;
    public publicValue: number;
    read() { return this./*#privateSpellingInside*/secrett; }
}
declare const spelling: Spelling;
spelling./*#privateSpellingOutside*/secrett;
spelling./*#protectedSpellingOutside*/protectedValu;
spelling./*#publicSpelling*/publicValu;
declare const impossible: { kind: "left"; other: MissingNever } & { kind: "right" };
impossible./*#neverDiscriminant*/absent;
class Left { private same: number; }
class Right { private same: number; }
declare const privateIntersection: Left & Right;
privateIntersection./*#neverPrivate*/absent;
interface Element {}
interface Node {}
interface HTMLCustomElement {}
interface OtherElement {}
declare const element: Element;
declare const node: Node;
declare const custom: HTMLCustomElement;
declare const other: OtherElement;
element./*#domElement*/absent;
node./*#domNode*/absent;
custom./*#domCustom*/absent;
other./*#domOther*/absent;
declare const domUnion: Element | Node;
domUnion./*#domUnion*/absent;
interface NonEmptyElement { present: number; }
declare const nonempty: NonEmptyElement;
nonempty./*#domNonempty*/absent;
interface BaseInterface {}
namespace InterfaceSpace { export interface Named {} }
class InvalidExtends extends /*#interfaceHeritage*/BaseInterface {}
class InvalidQualifiedExtends extends InterfaceSpace./*#qualifiedInterfaceHeritage*/Named {}
function nested() {
    type BaseInterface = string;
    class StillInterface extends /*#shadowedInterfaceHeritage*/BaseInterface {}
    return StillInterface;
}
