export {};
const /*#module*/moduleThis = this;
const /*#moduleArrow*/moduleArrow = () => this;
class Instance {
    field = this;
    get /*#getter*/object() { return { self: this }; }
    /*#method*/method() { return this; }
    /*#arrow*/arrow() { return () => this; }
    static /*#staticMethod*/method() { return this; }
    static /*#staticArrow*/arrow() { return () => this; }
    /*#parameter*/parameter(value = this) { return value; }
    /*#pattern*/pattern({ value = this } = {}) { return value; }
    nested() { function /*#nested*/nested() { return this; } return nested; }
}
function /*#explicit*/explicit(this: { tag: "explicit" }) { return this; }
function /*#ordinary*/ordinary() { return this; }
function /*#parameterOrdinary*/parameterOrdinary(value = this) { return value; }
const /*#contextual*/contextual: (this: { tag: "contextual" }) => unknown = function() { return this; };
const literal = { value: 1, /*#literal*/method() { return this.value; } };
const context: { value: number; method(): number } & ThisType<{ value: number }> = {
    value: 1, /*#thisType*/method() { return this.value; }
};
class Base { value = 1; }
class Derived extends Base { constructor() { const /*#beforeSuper*/beforeSuper = this; super(); } }
class NullBase extends null { constructor() { const /*#extendsNull*/extendsNull = this; } }
namespace Nested { export const /*#namespace*/namespaceThis = this; }
const outer: { nested: { method(): number } } & ThisType<{ value: number }> = {
    nested: { /*#outerThisType*/method() { return this.value; } }
};
