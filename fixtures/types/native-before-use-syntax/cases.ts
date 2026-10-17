export {};
interface Object {}
interface Function {}
interface Array<T> { length: number; [n:number]:T; }
interface ReadonlyArray<T> { readonly length:number; readonly [n:number]:T; }
declare let box: { a: number };
const earlier = "a";
box.a = 1;
box[earlier/*#earlier*/];
function deferred() {
    box.a = 1;
    box[later/*#deferred*/];
}
const later = "a";
(function () {
    box.a = 1;
    box[iifeLater/*#iife*/];
})();
const iifeLater = "a";
(async function () {
    box.a = 1;
    box[asyncLater/*#asyncIife*/];
})();
const asyncLater = "a";
(function* () {
    box.a = 1;
    box[generatorLater/*#generatorIife*/];
})();
const generatorLater = "a";
const arrow = () => {
    box.a = 1;
    box[arrowLater/*#arrow*/];
};
const arrowLater = "a";
function shadow() {
    const later = "a";
    box.a = 1;
    box[later/*#shadow*/];
}
class Computed {
    static key: "a";
    [Computed./*#computed*/key](): void {}
}
class Self {
    value: number = this./*#self*/value;
    other: () => number = () => this./*#selfArrow*/otherValue;
    otherValue: number;
}
class ParameterProperty {
    value: number = this./*#parameterProperty*/later;
    constructor(public later: number) {}
}
class StaticDeferred {
    static first: () => number = () => StaticDeferred./*#staticArrow*/later;
    static later: number;
}
class OptionalProperty {
    value: number | undefined = this./*#optional*/later;
    later?: number;
}
class AssertedProperty {
    value: number = this./*#asserted*/later;
    later!: number;
}
class Nested {
    static obj: { value: number };
    static value: number = Nested.obj./*#nested*/value;
}
class StaticOwnMethod {
    static value: unknown = StaticOwnMethod./*#method*/later;
    static later(): number { return 1; }
}
class Base { base: number; }
class Derived extends Base {
    value: number = this./*#inherited*/base;
}
declare const decorate: (value: unknown) => any;
@decorate(Decorated./*#decorator*/value)
class Decorated { static value: number; }
@decorate(() => DecoratedArrow./*#decoratorArrow*/value)
class DecoratedArrow { static value: number; }
