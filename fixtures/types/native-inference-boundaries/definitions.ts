export type NumericAlias<T> = { 0: T; };
export declare class PublicIndexClass { 0: number; }
export declare class PrivateIndexClass { private 0: number; }
export interface Covariant<T> { value: T; }
export interface Contravariant<T> { consume: (value: T) => void; }
export interface Invariant<T> { value: T; consume: (value: T) => void; }
export interface Method<T> { consume(value: T): void; }
export interface Independent<T> { tag: string; }
export interface RecursiveVariance<T> { value: T; next?: RecursiveVariance<T>; }
export interface DeclaredOut<out T> { value: T; }
export interface DeclaredIn<in T> { consume: (value: T) => void; }
export interface DeclaredInvariant<in out T> { value: T; }
export interface indexNumericSource { <T>(value: { [key: number]: T }): T; }
export interface indexNumericTarget { (value: { 0: "zero"; "01": MissingIndexExcluded; }): "zero"; }
export declare const indexNumericSourceValue: indexNumericSource;
export declare function indexNumericSelect(value: indexNumericTarget): "chosen";
export declare function indexNumericSelect(value: indexNumericSource): "same";
export interface indexNumericStringSource { <T>(value: { [key: number]: T }): T; }
export interface indexNumericStringTarget { (value: { "0": "zero"; "1.0": MissingIndexExcluded; }): "zero"; }
export declare const indexNumericStringSourceValue: indexNumericStringSource;
export declare function indexNumericStringSelect(value: indexNumericStringTarget): "chosen";
export declare function indexNumericStringSelect(value: indexNumericStringSource): "same";
export interface indexComputedSource { <T>(value: { [key: number]: T }): T; }
export interface indexComputedTarget { (value: { ["0"]: "zero"; ["01"]: MissingIndexExcluded; }): "zero"; }
export declare const indexComputedSourceValue: indexComputedSource;
export declare function indexComputedSelect(value: indexComputedTarget): "chosen";
export declare function indexComputedSelect(value: indexComputedSource): "same";
export interface indexInstantiatedSource { <T>(value: { [key: number]: T }): T; }
export interface indexInstantiatedTarget { (value: NumericAlias<"zero">): "zero"; }
export declare const indexInstantiatedSourceValue: indexInstantiatedSource;
export declare function indexInstantiatedSelect(value: indexInstantiatedTarget): "chosen";
export declare function indexInstantiatedSelect(value: indexInstantiatedSource): "same";
export interface indexOptionalSource { <T>(value: { [key: number]: T }): T; }
export interface indexOptionalTarget { (value: { 0?: "zero"; }): "zero"; }
export declare const indexOptionalSourceValue: indexOptionalSource;
export declare function indexOptionalSelect(value: indexOptionalTarget): "chosen";
export declare function indexOptionalSelect(value: indexOptionalSource): "same";
export interface indexStringSource { <T>(value: { [key: string]: T }): T; }
export interface indexStringTarget { (value: { 0: "zero"; word: "word"; }): "zero" | "word"; }
export declare const indexStringSourceValue: indexStringSource;
export declare function indexStringSelect(value: indexStringTarget): "chosen";
export declare function indexStringSelect(value: indexStringSource): "same";
export interface indexPublicClassSource { <T>(value: { [key: number]: T }): T; }
export interface indexPublicClassTarget { (value: PublicIndexClass): number; }
export declare const indexPublicClassSourceValue: indexPublicClassSource;
export declare function indexPublicClassSelect(value: indexPublicClassTarget): "chosen";
export declare function indexPublicClassSelect(value: indexPublicClassSource): "same";
export interface indexPrivateClassSource { <T>(value: { [key: number]: T }): T; }
export interface indexPrivateClassTarget { (value: PrivateIndexClass): number; }
export declare const indexPrivateClassSourceValue: indexPrivateClassSource;
export declare function indexPrivateClassSelect(value: indexPrivateClassTarget): "chosen";
export declare function indexPrivateClassSelect(value: indexPrivateClassSource): "same";
export interface varianceCovariantSource { <T>(value: Covariant<T>, direct: T): T; }
export interface varianceCovariantTarget { (value: Covariant<"a">, direct: "b"): "a" | "b"; }
export declare const varianceCovariantSourceValue: varianceCovariantSource;
export declare function varianceCovariantSelect(value: varianceCovariantTarget): "chosen";
export declare function varianceCovariantSelect(value: varianceCovariantSource): "same";
export interface varianceContravariantSource { <T>(value: Contravariant<T>, direct: T): T; }
export interface varianceContravariantTarget { (value: Contravariant<"a">, direct: "b"): "a" | "b"; }
export declare const varianceContravariantSourceValue: varianceContravariantSource;
export declare function varianceContravariantSelect(value: varianceContravariantTarget): "chosen";
export declare function varianceContravariantSelect(value: varianceContravariantSource): "same";
export interface varianceInvariantSource { <T>(value: Invariant<T>, direct: T): T; }
export interface varianceInvariantTarget { (value: Invariant<"a">, direct: "b"): "a" | "b"; }
export declare const varianceInvariantSourceValue: varianceInvariantSource;
export declare function varianceInvariantSelect(value: varianceInvariantTarget): "chosen";
export declare function varianceInvariantSelect(value: varianceInvariantSource): "same";
export interface varianceMethodSource { <T>(value: Method<T>, direct: T): T; }
export interface varianceMethodTarget { (value: Method<"a">, direct: "b"): "a" | "b"; }
export declare const varianceMethodSourceValue: varianceMethodSource;
export declare function varianceMethodSelect(value: varianceMethodTarget): "chosen";
export declare function varianceMethodSelect(value: varianceMethodSource): "same";
export interface varianceIndependentSource { <T>(value: Independent<T>, direct: T): T; }
export interface varianceIndependentTarget { (value: Independent<"a">, direct: "b"): "a" | "b"; }
export declare const varianceIndependentSourceValue: varianceIndependentSource;
export declare function varianceIndependentSelect(value: varianceIndependentTarget): "chosen";
export declare function varianceIndependentSelect(value: varianceIndependentSource): "same";
export interface varianceRecursiveSource { <T>(value: RecursiveVariance<T>, direct: T): T; }
export interface varianceRecursiveTarget { (value: RecursiveVariance<"a">, direct: "b"): "a" | "b"; }
export declare const varianceRecursiveSourceValue: varianceRecursiveSource;
export declare function varianceRecursiveSelect(value: varianceRecursiveTarget): "chosen";
export declare function varianceRecursiveSelect(value: varianceRecursiveSource): "same";
export interface varianceDeclaredOutSource { <T>(value: DeclaredOut<T>, direct: T): T; }
export interface varianceDeclaredOutTarget { (value: DeclaredOut<"a">, direct: "b"): "a" | "b"; }
export declare const varianceDeclaredOutSourceValue: varianceDeclaredOutSource;
export declare function varianceDeclaredOutSelect(value: varianceDeclaredOutTarget): "chosen";
export declare function varianceDeclaredOutSelect(value: varianceDeclaredOutSource): "same";
export interface varianceDeclaredInSource { <T>(value: DeclaredIn<T>, direct: T): T; }
export interface varianceDeclaredInTarget { (value: DeclaredIn<"a">, direct: "b"): "a" | "b"; }
export declare const varianceDeclaredInSourceValue: varianceDeclaredInSource;
export declare function varianceDeclaredInSelect(value: varianceDeclaredInTarget): "chosen";
export declare function varianceDeclaredInSelect(value: varianceDeclaredInSource): "same";
export interface varianceDeclaredInvariantSource { <T>(value: DeclaredInvariant<T>, direct: T): T; }
export interface varianceDeclaredInvariantTarget { (value: DeclaredInvariant<"a">, direct: "b"): "a" | "b"; }
export declare const varianceDeclaredInvariantSourceValue: varianceDeclaredInvariantSource;
export declare function varianceDeclaredInvariantSelect(value: varianceDeclaredInvariantTarget): "chosen";
export declare function varianceDeclaredInvariantSelect(value: varianceDeclaredInvariantSource): "same";
export interface BrokenVariance<T> { value: T; broken: MissingVarianceDependency; }
export interface varianceFailureSource { <T>(value: BrokenVariance<T>): T; }
export interface varianceFailureTarget { (value: BrokenVariance<number>): number; }
export declare const varianceFailureSourceValue: varianceFailureSource;
export declare function varianceFailureSelect(value: varianceFailureTarget): "chosen";
export declare function varianceFailureSelect(value: varianceFailureSource): "same";
export declare function lateValue<T extends { value: U }, U = number>(): T;
export declare function lateFailureValue<T extends { value: U }, U = MissingLateDefault>(): T;
