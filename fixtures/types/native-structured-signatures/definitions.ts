// Each selected scalar forces an actual overloaded call comparison.
export interface ReturnSource { (): MissingReturn; }
export interface VoidTarget { (): void; }
export interface AnyTarget { (): any; }
export interface UnknownTarget { (): unknown; }
export interface NumberTarget { (): number; }
export interface NumberSource { (): number; }
export interface MissingTarget { (): MissingTargetReturn; }
export interface ArityEarlySource { (early: MissingEarly, trailing: number): void; }
export interface ArityTailSource { (early: number, trailing: MissingTail): void; }
export interface OneTarget { (value: number): void; }
export interface ExtraTarget { (unused: MissingExtra): void; }
export interface ThisEarlySource { (this: MissingThis, early: number, trailing: number): void; }
export interface RequiredVoidSource { (value: void): void; }
export interface RequiredUndefinedSource { (value: undefined): void; }
export interface OptionalSource { (value?: number | undefined): void; }
export interface PropertyFirstSource { gate: number; (): MissingHiddenReturn; }
export interface PropertyFirstTarget { gate: string; (): number; }
export interface CallFirstSource { (): number; new (): MissingHiddenConstruct; }
export interface CallFirstTarget { (): string; new (): number; }
export interface ConstructFirstSource { new (): number; [key: string]: any; }
export interface ConstructFirstTarget { new (): string; [key: string]: number; }
export interface IndexInventorySource { (): number; [key: string]: MissingInventory; }
export interface CallMismatchTarget { (): string; }
export interface ConstructSource { new (): MissingNewReturn; }
export interface ConstructVoidTarget { new (): void; }
export interface ConstructAnyTarget { new (): any; }
export interface ConstructUnknownTarget { new (): unknown; }
export interface FunctionVarianceSource { fn: (value: "a") => void; }
export interface FunctionVarianceTarget { fn: (value: string) => void; }
export interface MethodVarianceSource { fn(value: "a"): void; }
export interface MethodVarianceTarget { fn(value: string): void; }
export interface CallbackSource { (callback: (value: string) => void): void; }
export interface CallbackTarget { (callback: (value: "a") => void): void; }
export interface OverloadSource { (value: number): number; (value: string): string; }
export interface OverloadTarget { (value: string): string; }
export interface OverloadBothTarget { (value: string): string; (value: number): number; }
export interface OverloadMissingTarget { (value: string): string; (value: boolean): boolean; }
export interface PredicateSource { (x: unknown, y: unknown): x is string; }
export interface PredicateTarget { (x: unknown, y: unknown): x is string; }
export interface PredicateIndexTarget { (x: unknown, y: unknown): y is string; }
export interface BooleanSource { (x: unknown, y: unknown): boolean; }
export interface MissingPredicateSource { (x: unknown, y: unknown): x is MissingPredicate; }
export interface GenericContextSource { <T>(value: T): T; }
export interface GenericContextTarget { (value: number): number; }
export interface GenericEraseSource { <T extends MissingErasedConstraint>(value: T): number; (value: string): string; }
export type AbstractSource = abstract new () => number;
export type ConcreteTarget = new () => number;
export declare const returnVoidSource: ReturnSource;
export declare function returnVoidSelect(value: VoidTarget): "chosen";
export declare function returnVoidSelect(value: ReturnSource): "same";
export declare const returnAnySource: ReturnSource;
export declare function returnAnySelect(value: AnyTarget): "chosen";
export declare function returnAnySelect(value: ReturnSource): "same";
export declare const returnUnknownSource: ReturnSource;
export declare function returnUnknownSelect(value: UnknownTarget): "chosen";
export declare function returnUnknownSelect(value: ReturnSource): "same";
export declare const returnNumberSource: ReturnSource;
export declare function returnNumberSelect(value: NumberTarget): "chosen";
export declare function returnNumberSelect(value: ReturnSource): "same";
export declare const targetReturnSource: NumberSource;
export declare function targetReturnSelect(value: MissingTarget): "chosen";
export declare function targetReturnSelect(value: NumberSource): "same";
export declare const identitySource: ReturnSource;
export declare function identitySelect(value: ReturnSource): "chosen";
export declare function identitySelect(value: ReturnSource): "same";
export declare const arityEarlySource: ArityEarlySource;
export declare function arityEarlySelect(value: OneTarget): "chosen";
export declare function arityEarlySelect(value: ArityEarlySource): "same";
export declare const arityTailSource: ArityTailSource;
export declare function arityTailSelect(value: OneTarget): "chosen";
export declare function arityTailSelect(value: ArityTailSource): "same";
export declare const extraTargetSource: VoidTarget;
export declare function extraTargetSelect(value: ExtraTarget): "chosen";
export declare function extraTargetSelect(value: VoidTarget): "same";
export declare const thisEarlySource: ThisEarlySource;
export declare function thisEarlySelect(value: OneTarget): "chosen";
export declare function thisEarlySelect(value: ThisEarlySource): "same";
export declare const requiredVoidSource: RequiredVoidSource;
export declare function requiredVoidSelect(value: VoidTarget): "chosen";
export declare function requiredVoidSelect(value: RequiredVoidSource): "same";
export declare const requiredUndefinedSource: RequiredUndefinedSource;
export declare function requiredUndefinedSelect(value: VoidTarget): "chosen";
export declare function requiredUndefinedSelect(value: RequiredUndefinedSource): "same";
export declare const optionalSource: OptionalSource;
export declare function optionalSelect(value: VoidTarget): "chosen";
export declare function optionalSelect(value: OptionalSource): "same";
export declare const propertyFirstSource: PropertyFirstSource;
export declare function propertyFirstSelect(value: PropertyFirstTarget): "chosen";
export declare function propertyFirstSelect(value: PropertyFirstSource): "same";
export declare const callFirstSource: CallFirstSource;
export declare function callFirstSelect(value: CallFirstTarget): "chosen";
export declare function callFirstSelect(value: CallFirstSource): "same";
export declare const constructFirstSource: ConstructFirstSource;
export declare function constructFirstSelect(value: ConstructFirstTarget): "chosen";
export declare function constructFirstSelect(value: ConstructFirstSource): "same";
export declare const indexInventorySource: IndexInventorySource;
export declare function indexInventorySelect(value: CallMismatchTarget): "chosen";
export declare function indexInventorySelect(value: IndexInventorySource): "same";
export declare const constructVoidSource: ConstructSource;
export declare function constructVoidSelect(value: ConstructVoidTarget): "chosen";
export declare function constructVoidSelect(value: ConstructSource): "same";
export declare const constructAnySource: ConstructSource;
export declare function constructAnySelect(value: ConstructAnyTarget): "chosen";
export declare function constructAnySelect(value: ConstructSource): "same";
export declare const constructUnknownSource: ConstructSource;
export declare function constructUnknownSelect(value: ConstructUnknownTarget): "chosen";
export declare function constructUnknownSelect(value: ConstructSource): "same";
export declare const functionVarianceSource: FunctionVarianceSource;
export declare function functionVarianceSelect(value: FunctionVarianceTarget): "chosen";
export declare function functionVarianceSelect(value: FunctionVarianceSource): "same";
export declare const methodVarianceSource: MethodVarianceSource;
export declare function methodVarianceSelect(value: MethodVarianceTarget): "chosen";
export declare function methodVarianceSelect(value: MethodVarianceSource): "same";
export declare const callbackVarianceSource: CallbackSource;
export declare function callbackVarianceSelect(value: CallbackTarget): "chosen";
export declare function callbackVarianceSelect(value: CallbackSource): "same";
export declare const overloadSecondSource: OverloadSource;
export declare function overloadSecondSelect(value: OverloadTarget): "chosen";
export declare function overloadSecondSelect(value: OverloadSource): "same";
export declare const overloadAllSource: OverloadSource;
export declare function overloadAllSelect(value: OverloadBothTarget): "chosen";
export declare function overloadAllSelect(value: OverloadSource): "same";
export declare const overloadMismatchSource: OverloadSource;
export declare function overloadMismatchSelect(value: OverloadMissingTarget): "chosen";
export declare function overloadMismatchSelect(value: OverloadSource): "same";
export declare const predicateSameSource: PredicateSource;
export declare function predicateSameSelect(value: PredicateTarget): "chosen";
export declare function predicateSameSelect(value: PredicateSource): "same";
export declare const predicateIndexSource: PredicateSource;
export declare function predicateIndexSelect(value: PredicateIndexTarget): "chosen";
export declare function predicateIndexSelect(value: PredicateSource): "same";
export declare const predicateBooleanSource: BooleanSource;
export declare function predicateBooleanSelect(value: PredicateTarget): "chosen";
export declare function predicateBooleanSelect(value: BooleanSource): "same";
export declare const predicateMissingSource: MissingPredicateSource;
export declare function predicateMissingSelect(value: PredicateTarget): "chosen";
export declare function predicateMissingSelect(value: MissingPredicateSource): "same";
export declare const genericContextSource: GenericContextSource;
export declare function genericContextSelect(value: GenericContextTarget): "chosen";
export declare function genericContextSelect(value: GenericContextSource): "same";
export declare const genericEraseSource: GenericEraseSource;
export declare function genericEraseSelect(value: GenericContextTarget): "chosen";
export declare function genericEraseSelect(value: GenericEraseSource): "same";
export declare const abstractConcreteSource: AbstractSource;
export declare function abstractConcreteSelect(value: ConcreteTarget): "chosen";
export declare function abstractConcreteSelect(value: AbstractSource): "same";
export declare const concreteAbstractSource: ConcreteTarget;
export declare function concreteAbstractSelect(value: AbstractSource): "chosen";
export declare function concreteAbstractSelect(value: ConcreteTarget): "same";
