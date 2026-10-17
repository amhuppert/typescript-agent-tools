export {};
declare function take(value: { required: string }): "taken";
declare function consume(value: boolean): "checked";
declare function observe(value: unknown): "observed";
const first = { value: 1 };
const second = { value: 2 };
take(first /*#cold*/);
observe((take(first), take(first)) /*#warm*/);
observe((take(first), take(second)) /*#different*/);
declare const lazy: { hidden?: MissingCache };
const recovered = { value: 1, lazy };
take(recovered /*#recoveredCold*/);
observe((take(recovered), take(recovered)) /*#recoveredWarm*/);
namespace Left { class Named { private left!: void; } export const value = { item: new Named() }; }
namespace Right { class Named { private right!: void; } export const value = { item: new Named() }; }
consume(Left.value === Right.value /*#sameBytes*/);
consume(Left.value === Left.value /*#equalControl*/);
const long = {
    alpha: "alpha", beta: "beta", gamma: "gamma", delta: "delta", epsilon: "epsilon",
    zeta: "zeta", eta: "eta", theta: "theta", iota: "iota", kappa: "kappa",
    lambda: "lambda", mu: "mu", nu: "nu", xi: "xi", omicron: "omicron",
    pi: "pi", rho: "rho", sigma: "sigma", tau: "tau", upsilon: "upsilon",
    propertyNumber00: "value00",
    propertyNumber01: "value01",
    propertyNumber02: "value02",
    propertyNumber03: "value03",
    propertyNumber04: "value04",
    propertyNumber05: "value05",
    propertyNumber06: "value06",
    propertyNumber07: "value07",
    propertyNumber08: "value08",
    propertyNumber09: "value09",
    propertyNumber10: "value10",
    propertyNumber11: "value11",
    propertyNumber12: "value12",
    propertyNumber13: "value13",
    propertyNumber14: "value14",
    propertyNumber15: "value15",
    propertyNumber16: "value16",
    propertyNumber17: "value17",
    propertyNumber18: "value18",
    propertyNumber19: "value19",

};
take(long /*#longCold*/);
observe((take(long), take(long)) /*#longWarm*/);
class Self { get object() { return { self: this }; } }
declare const instance: Self;
take(instance.object /*#thisCold*/);
observe((take(instance.object), take(instance.object)) /*#thisWarm*/);
declare const recursive: { value: number; next?: typeof recursive };
take(recursive /*#recursive*/);
class Polymorphic {
    check(value: { self: this }) {
        take(value /*#reportedCold*/);
        observe((take(value), take(value)) /*#reportedWarm*/);
    }
}
