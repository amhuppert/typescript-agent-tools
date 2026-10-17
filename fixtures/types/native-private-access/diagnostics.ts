export {};
class Outer {
    #same: number = 1;
    readonly #fixed: number = 1;
    static #static = 1;
    missing(other: Outer) { return other./*#missingName*/#absent; }
    errorReceiver() { return MissingReceiver./*#errorReceiver*/#same; }
    readonlyWrite() { this./*#readonlyWrite*/#fixed = 2; }
    instanceStatic() { return this./*#instanceStatic*/#static; }
    static staticInstance() { return Outer./*#staticInstance*/#same; }
    anyMissing(other: any) { return other./*#anyMissing*/#absent; }
    never(other: never) { return other./*#neverReceiver*/#same; }
    union(other: Outer | { ordinary: number }) { return other./*#unionMissing*/#same; }
    unrelated(other: Unrelated) { return other./*#unrelated*/#same; }
    nested(other: Outer) {
        class Inner { #same = false; read() { return other./*#shadowed*/#same; } }
        return Inner;
    }
    nestedDisplay(other: Outer & { hidden: MissingShadowDisplay }) {
        class Inner { #same = false; read() { return other./*#shadowedDisplay*/#same; } }
        return Inner;
    }
    missingDisplay(other: { hidden: MissingNonexistentDisplay }) {
        return other./*#nonexistentDisplay*/#same;
    }
}
class Unrelated { #same: string = "unrelated"; }
declare const outer: Outer;
declare const anyOutside: any;
const outside = outer./*#outsideClass*/#same;
const outsideAny = anyOutside./*#outsideAny*/#absent;
