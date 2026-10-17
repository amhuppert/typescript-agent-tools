export {};
class Plain { constructor(value = 0) {} }
class Generic<T> { constructor(value: T) {} }
class Defaulted<T = string> { constructor(value: T) {} }
class Pair<A, B> { constructor(first: A, second: B) {} }
class Overloaded {
  constructor(value: number);
  constructor(value: string);
  constructor(value: unknown) {}
}
declare const FunctionBase: new <T>(value: T) => { value: T };
declare const AnyBase: any;
declare const LazyReturn: new () => { latent: MissingUnusedReturn };
class Simple extends Plain { constructor() { const /*#plain*/plain = super(); } }
class Applied extends Generic<number> { constructor() { const /*#generic*/generic = super(1); } }
class Default extends Defaulted { constructor() { const /*#defaulted*/defaulted = super("ok"); } }
class DefaultOverride extends Defaulted<number> { constructor() { const /*#defaultOverride*/defaultOverride = super(1); } }
class Overload extends Overloaded { constructor() { const /*#overload*/overload = super("ok"); } }
class ConstructorFunction extends FunctionBase<number> { constructor() { const /*#classLike*/classLike = super(1); } }
class AnyHealthy extends AnyBase { constructor() { const /*#anyHealthy*/anyHealthy = super(1); } }
class NoReturnDisplay extends LazyReturn { constructor() { const /*#lazyReturn*/lazyReturn = super(); } }
class FailedArgument extends Plain { constructor() { const /*#argumentRecovery*/argumentRecovery = super(missingPlainArgument); } }
class AnyRecovered extends AnyBase { constructor() { const /*#anyRecovery*/anyRecovery = super(missingAnyArgument); } }
class NoBase { constructor() { const /*#noBase*/noBase = super(missingNoBaseArgument); } }
class NullBase extends null { constructor() { const /*#nullBase*/nullBase = super(missingNullArgument); } }
class BadApplied extends Generic<MissingApplied> { constructor() { const /*#appliedRecovery*/appliedRecovery = super(missingAppliedArgument); } }
class BadPair extends Pair<MissingPairFirst, MissingPairSecond> { constructor() { const /*#pairRecovery*/pairRecovery = super(missingPairFirstArgument, missingPairSecondArgument); } }
class WrongArity extends Generic<number, string> { constructor() { const /*#wrongArity*/wrongArity = super(missingArityArgument); } }
class WrongFunctionArity extends FunctionBase<number, string> { constructor() { const /*#wrongFunctionArity*/wrongFunctionArity = super(missingFunctionArityArgument); } }
class Mismatch extends Generic<string> { constructor() { const /*#overloadRecovery*/overloadRecovery = super(1); } }
class MissingBase extends MissingSuperConstructor { constructor() { const /*#baseRecovery*/baseRecovery = super(missingBaseArgument); } }
