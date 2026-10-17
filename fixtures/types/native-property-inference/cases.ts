class Base<T> {
  value!: T;
  static value: string;
  optional?: T;
}
class Derived extends Base<number> {
  declare /*#inherited*/value;
  declare /*#optional*/optional;
  declare /*#missing*/absent;
  static declare /*#staticSameName*/value;
}
class NoBase {
  declare /*#noBase*/value;
  static declare /*#staticNoBase*/value;
}
class Empty {
  /*#emptyConstructor*/value;
  constructor() {}
}
class Assigned {
  /*#assignedConstructor*/value;
  constructor() { this.value = 1; }
}
class StaticEmpty {
  static /*#emptyStatic*/value;
  static {}
}
class StaticAssigned {
  static /*#assignedStatic*/value;
  static { this.value = 1; }
}
class InstanceWithStaticBlock extends Base<number> {
  declare /*#instanceWithStaticBlock*/value;
  static {}
}
class StaticWithConstructor extends Base<number> {
  static declare /*#staticWithConstructor*/value;
  constructor() { super(); }
}
class DeclareWithConstructor extends Base<number> {
  declare /*#declareWithConstructor*/value;
  constructor() { super(); }
}
class StaticDeclareWithBlock extends Base<number> {
  static declare /*#staticDeclareWithBlock*/value;
  static {}
}
