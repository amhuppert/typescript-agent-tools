export {};
class Base {
  x: number = 1;
  static x: number = 2;
  method(): number { return this.x; }
  static method(): number { return this.x; }
}
class Derived extends Base {
  constructor(value = 0) {
    /*#beforeCall*/super.x;
    /*#constructorCall*/super();
    /*#constructorProperty*/super.x;
  }
  method(): number { return /*#instanceMethod*/super.x; }
  get value(): number { return /*#instanceGetter*/super.x; }
  set value(value: number) { /*#instanceSetter*/super.x = value; }
  field = /*#instanceField*/super.x;
  arrow() { return () => () => /*#arrowProperty*/super.x; }
  nested() { return function () { return /*#nestedFunction*/super.x; }; }
  static method(): number { return /*#staticMethod*/super.x; }
  static get value(): number { return /*#staticGetter*/super.x; }
  static set value(value: number) { /*#staticSetter*/super.x = value; }
  static field = /*#staticField*/super.x;
  static { /*#staticBlock*/super.x; }
}
class Argument extends Base {
  constructor(value = /*#constructorArgument*/super.x) { super(); }
}
class ArrowCall extends Base {
  constructor() { const deferred = () => /*#arrowCall*/super(); super(); }
}
class NoBase { method() { return /*#noBase*/super.x; } }
class NullBase extends null {
  constructor() { /*#nullCall*/super(); }
  method() { return /*#nullProperty*/super.x; }
  static method() { return /*#nullStatic*/super.x; }
}
const object = {
  method() { return /*#objectMethod*/super.x; },
  get value() { return /*#objectGetter*/super.x; },
  set value(value: number) { /*#objectSetter*/super.x = value; },
  arrow() { return () => /*#objectArrow*/super.x; },
};
function outside() { return /*#outside*/super.x; }
class Computed extends Base { [/*#computedName*/super.x]() {} }
class Broken extends MissingSuperBase { method() { return /*#brokenBase*/super.x; } }
class Generic<T> extends Base { method() { return /*#genericClass*/super.x; } }
