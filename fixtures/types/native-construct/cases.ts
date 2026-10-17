export {};
declare class Box<T> { value: T; constructor(value: T); }
const publicCase = /*#public*/new Box(1).value;
const explicitCase = /*#explicit*/new Box<string>("x").value;
declare const newable: {new (value: number): {readonly tag: "newable"}};
const newableCase = /*#newable*/new newable(1).tag;
declare const anyCtor: any;
const anyCase = /*#any*/new anyCtor(1);
const anyBadCase = /*#anyBad*/new anyCtor(MissingArgument);
declare function callOnly(value: number): string;
const callOnlyCase = /*#callOnly*/new callOnly(1);
declare function callMissing(): MissingReturn;
const callMissingCase = /*#callMissing*/new callMissing();
class Private { private constructor() {} static make() { return /*#privateInside*/new Private(); } static nested() { return class Nested { make() { return /*#privateNested*/new Private(); } }; } }
const privateOutside = /*#privateOutside*/new Private();
class Protected { protected constructor() {} }
class Derived extends Protected { static make() { return /*#protectedInside*/new Protected(); } }
class Grandchild extends Derived { static make() { return /*#protectedGrandchild*/new Protected(); } }
const protectedOutside = /*#protectedOutside*/new Protected();
const inheritedOutside = /*#inheritedOutside*/new Derived();
abstract class Abstract { constructor() {} }
const abstractCase = /*#abstract*/new Abstract();
declare const abstractCtor: abstract new () => { value: number };
const abstractSignatureCase = /*#abstractSignature*/new abstractCtor();
declare const voidCtor: new (value: void) => { value: "void" };
const voidParenCase = /*#voidParen*/new voidCtor().value;
const voidBareCase = /*#voidBare*/new voidCtor;
declare const noArgs: new () => { value: "empty" };
const bareCase = /*#bare*/new noArgs;
const invalidArgument = /*#argumentFailure*/new Box(MissingConstructorArgument);
const invalidAbstractArgument = /*#abstractArgument*/new Abstract(MissingAbstractArgument);
declare abstract class GenericAbstract<T> { value: T; constructor(value: T); }
const genericAbstractCase = /*#genericAbstract*/new GenericAbstract(1);
const checkedPrivate = new Private()./*#checkedPrivate*/value;
const checkedAbstract = new Abstract()./*#checkedAbstract*/value;
declare const notConstructor: number;
const noConstructorCase = /*#notConstructor*/new notConstructor();
declare function receiverCall(this: { required: true }, value: number): void;
const receiverCallCase = /*#receiverCall*/new receiverCall(1);
declare function receiverMissing(this: MissingThis): void;
const receiverMissingCase = /*#receiverMissing*/new receiverMissing();
declare const unionAbstract: (abstract new () => { union: true }) | (new () => { union: true });
const unionAbstractCase = /*#unionAbstract*/new unionAbstract();
