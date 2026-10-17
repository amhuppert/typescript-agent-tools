export {};
declare function ordinary(): void;
function parenthesized() { let value = 1; (ordinary)(); /*parenthesized*/ value; }
function comma() { let value = 1; (0, ordinary)(); /*comma*/ value; }
function nonnull() { let value = 1; ordinary!(); /*nonnull*/ value; }
function element() { let value = 1; ordinary["x"](); /*element*/ value; }
function nestedCall() { let value = 1; ordinary()(); /*nested-call*/ value; }
function arrow() { let value = 1; (() => 1)(); /*arrow*/ value; }
function conditional() { let value = 1; (true ? ordinary : ordinary)(); /*conditional*/ value; }
function number() { let value = 1; (1)(); /*number*/ value; }
function string() { let value = 1; ("x")(); /*string*/ value; }
function object() { let value = 1; ({})(); /*object*/ value; }
function pureMatching() { let value = 1; (typeof value === "number" ? ordinary : ordinary)(); /*pure-matching*/ value; }
function pureUnknown() { let value = 1; (Missing ? ordinary : ordinary)(); /*pure-unknown*/ value; }
function pureNested() { let value = 1; (typeof value === "number" ? (Missing ? ordinary : ordinary) : ordinary)(); /*pure-nested*/ value; }
function assignedBranches() { let value = 1; (true ? value = 2 : value = 3); /*assigned-branches*/ value; }

declare const methodTarget: { run(): void };
declare const propertyTarget: { run: () => void };
function dottedMethod() { let value = 1; methodTarget.run(); /*dotted-method*/ value; }
function dottedProperty() { let value = 1; propertyTarget.run(); /*dotted-property*/ value; }
function dottedMissing() { let value = 1; methodTarget.missing(); /*dotted-missing*/ value; }
function dottedUnannotated() { const target = { run() { return Missing; } }; let value = 1; target.run(); /*dotted-unannotated*/ value; }
function dottedParentheses() { let value = 1; ((methodTarget)).run(); /*dotted-parentheses*/ value; }
function dottedThis(this: { run(): void }) { let value = 1; this.run(); /*dotted-this*/ value; }
function dottedArrowThis(this: { run(): void }) { return () => { let value = 1; this.run(); /*dotted-arrow-this*/ value; }; }
class EffectHost {
 run(): void {}
 method(): void { let value = 1; this.run(); /*dotted-class-this*/ value; }
 static run(): void {}
 static method(): void { let value = 1; this.run(); /*dotted-static-this*/ value; }
}
