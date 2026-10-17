interface Object {}
interface Function {}
interface Array<T> { length: number; [n: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }
interface String { readonly length: number }
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}

type Healthy = { tag: "a"; consume: (value: 1) => void } | { tag: "b"; consume: (value: 2) => void };
const selected: Healthy = { tag: "a", consume(/*#selected*/value) {} };
const unknownTag: Healthy = { tag: missingTag, consume(/*#unknownTag*/value) {} };
type Broken = { tag: "a"; consume: (value: MissingFirst) => void } | { tag: "b"; consume: (value: MissingSecond) => void };
const broken: Broken = { tag: missingBrokenTag, consume(/*#broken*/value) {} };
declare const tag: MissingShorthand;
const shorthand: Broken = { tag, consume(/*#shorthand*/value) {} };
type Optional = { tag?: "a"; consume: (value: 3) => void } | { tag: "b"; consume: (value: 4) => void };
const omitted: Optional = { consume(/*#omitted*/value) {} };
const present: Optional = { tag: undefined, consume(/*#present*/value) {} };
type BothOptional = { tag?: "a"; consume: (value: 5) => void } | { tag?: "b"; consume: (value: 6) => void };
const bothOmitted: BothOptional = { consume(/*#bothOmitted*/value) {} };
type TwoMethods = { tag: "a"; first: (value: 7) => void; second: (value: 8) => void } | { tag: "b"; first: (value: 9) => void; second: (value: 10) => void };
const twoMethods: TwoMethods = { tag: "a", first(/*#firstMethod*/value) {}, second(/*#secondMethod*/value) {} };
interface Branch0 { tag: "v0"; consume: (value: 0) => void; }
interface Branch1 { tag: "v1"; consume: (value: 1) => void; }
interface Branch2 { tag: "v2"; consume: (value: 2) => void; }
interface Branch3 { tag: "v3"; consume: (value: 3) => void; }
interface Branch4 { tag: "v4"; consume: (value: 4) => void; }
interface Branch5 { tag: "v5"; consume: (value: 5) => void; }
interface Branch6 { tag: "v6"; consume: (value: 6) => void; }
interface Branch7 { tag: "v7"; consume: (value: 7) => void; }
interface Branch8 { tag: "v8"; consume: (value: 8) => void; }
interface Branch9 { tag: "v9"; consume: (value: 9) => void; }
interface Branch10 { tag: "v10"; consume: (value: 10) => void; }
interface Duplicate { tag: "v0"; consume: (value: 90) => void; }
type Target9 = Branch0 | Branch1 | Branch2 | Branch3 | Branch4 | Branch5 | Branch6 | Branch7 | Branch8;
const at9Match: Target9 = { tag: "v0", consume(/*#at9Match*/value) {} };
const at9Miss: Target9 = { tag: "missing", consume(/*#at9Miss*/value) {} };
const at9Duplicate: Target9 | Duplicate = { tag: "v0", consume(/*#at9Duplicate*/value) {} };
type Target10 = Branch0 | Branch1 | Branch2 | Branch3 | Branch4 | Branch5 | Branch6 | Branch7 | Branch8 | Branch9;
const at10Match: Target10 = { tag: "v0", consume(/*#at10Match*/value) {} };
const at10Miss: Target10 = { tag: "missing", consume(/*#at10Miss*/value) {} };
const at10Duplicate: Target10 | Duplicate = { tag: "v0", consume(/*#at10Duplicate*/value) {} };
type Target11 = Branch0 | Branch1 | Branch2 | Branch3 | Branch4 | Branch5 | Branch6 | Branch7 | Branch8 | Branch9 | Branch10;
const at11Match: Target11 = { tag: "v0", consume(/*#at11Match*/value) {} };
const at11Miss: Target11 = { tag: "missing", consume(/*#at11Miss*/value) {} };
const at11Duplicate: Target11 | Duplicate = { tag: "v0", consume(/*#at11Duplicate*/value) {} };
const /*#healthy*/healthy = 44 as const;
