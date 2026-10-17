declare class Base<T> {
  constructor(input: T);
  base: T;
  protected guarded: boolean;
  private hidden: number;
  static inheritedStatic: string;
  static replaced: string;
}
declare class Derived extends Base<string> {
  own: boolean;
  static replaced: "derived";
}
declare const derivedInstance: Derived;
declare const derivedStatic: typeof Derived;

declare class Explicit extends Base<number> {
  constructor(input: boolean, extra?: number);
  own: string;
}
declare const explicitInstance: Explicit;
declare const explicitStatic: typeof Explicit;

declare abstract class Abstract extends Base<boolean> {}
declare class Concrete extends Abstract {}
declare const abstractStatic: typeof Abstract;
declare const concreteStatic: typeof Concrete;
declare function concreteChoice(value: new(input: boolean) => Abstract): "concrete";
declare function concreteChoice(value: abstract new(input: boolean) => Abstract): "abstract";
const abstractResult = concreteChoice(Abstract);
const concreteResult = concreteChoice(Concrete);

declare class Plain { own: number; }
declare class NullBase extends null { own: string; }
declare const plainInstance: Plain;
declare const plainStatic: typeof Plain;
declare const nullInstance: NullBase;
declare const nullStatic: typeof NullBase;

declare const AnyBase: any;
declare class AnyDerived extends AnyBase { own: boolean; }
declare const anyInstance: AnyDerived;
declare const anyStatic: typeof AnyDerived;

declare const ClassLike: { new<T>(input: T): { inherited: T }; shared: string };
declare class LikeDerived extends ClassLike<number> { own: boolean; }
declare const likeInstance: LikeDerived;
declare const likeStatic: typeof LikeDerived;

interface MergeBase { interfaceBase: string; }
declare class Merged extends Base<number> { own: boolean; }
interface Merged extends MergeBase { interfaceOwn: number; }
declare const mergedInstance: Merged;
declare const mergedStatic: typeof Merged;

declare class GenericDerived<U = number> extends Base<U> { own: U; }
declare const genericInstance: GenericDerived<string>;
declare const genericStatic: typeof GenericDerived;
