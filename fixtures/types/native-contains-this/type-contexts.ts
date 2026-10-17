type Invalid = /*#invalidAlias*/this;
interface NestedInvalid { value: { nested: /*#invalidNested*/this }; }
interface GenericInvalid<T extends /*#invalidConstraint*/this> { value: T; }
class StaticInvalid { static value: /*#invalidStatic*/this; }
class ConstructorInvalid { constructor(input: /*#invalidConstructor*/this) {} }
function ordinary(): /*#invalidFunction*/this { throw 0; }
class ClassMember { value: /*#validClass*/this; }
class ConstructorBody { constructor() { let input: /*#validBody*/this; } }
class ArrowMember { value = (input: /*#validArrow*/this) => input; }
class StaticArrow { static value = (input: /*#invalidStaticArrow*/this) => input; }
class NestedFunction { method() { function nested(): /*#invalidNestedFunction*/this { throw 0; } } }
class ComputedName { [(input: /*#invalidComputed*/this) => input]: number; }
interface Merged { value: number; }
interface Merged { one: /*#validMergedOne*/this; two: /*#validMergedTwo*/this; }
