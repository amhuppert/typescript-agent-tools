export interface Source { gate: number; latent: MissingSource; }
export interface UnknownTarget { gate: number; latent: unknown; }
export interface AnyTarget { gate: number; latent: any; }
export interface NumberTarget { gate: number; latent: number; }
export interface MismatchFirstTarget { gate: string; latent: number; }
export interface MismatchLastTarget { latent: number; gate: string; }
export interface UnmatchedTarget { absent: number; latent: number; }
export interface OptionalSource { gate: number; latent?: MissingOptional; }
export interface OptionalUnknownTarget { gate: number; latent?: unknown; }
export interface RequiredUnknownTarget { gate: number; latent: unknown; }
export interface SourceIndex { gate: number; [key: string]: MissingSourceIndex; }
export interface TargetIndex { gate: string; [key: string]: MissingTargetIndex; }
export interface ValidIndexTarget { gate: string; [key: string]: unknown; }
export interface ReadonlyTarget { readonly gate: number; readonly latent: unknown; }
export declare const source: Source;
export declare const optionalSource: OptionalSource;
export declare const sourceIndex: SourceIndex;
export declare function unknownSelect(value: UnknownTarget): "chosen";
export declare function unknownSelect(value: never): "fallback";
export declare function anySelect(value: AnyTarget): "chosen";
export declare function anySelect(value: never): "fallback";
export declare function numberSelect(value: NumberTarget): "chosen";
export declare function numberSelect(value: never): "fallback";
export declare function mismatchFirstSelect(value: MismatchFirstTarget): "first";
export declare function mismatchFirstSelect(value: Source): "same";
export declare function mismatchLastSelect(value: MismatchLastTarget): "first";
export declare function mismatchLastSelect(value: Source): "same";
export declare function identitySelect(value: Source): "identity";
export declare function identitySelect(value: never): "fallback";
export declare function unmatchedSelect(value: UnmatchedTarget): "first";
export declare function unmatchedSelect(value: Source): "same";
export declare function optionalUnknownSelect(value: OptionalUnknownTarget): "chosen";
export declare function optionalUnknownSelect(value: never): "fallback";
export declare function optionalRequiredSelect(value: RequiredUnknownTarget): "first";
export declare function optionalRequiredSelect(value: OptionalSource): "same";
export declare function sourceIndexSelect(value: MismatchFirstTarget): "first";
export declare function sourceIndexSelect(value: SourceIndex): "same";
export declare function targetIndexSelect(value: TargetIndex): "first";
export declare function targetIndexSelect(value: Source): "same";
export declare function validIndexSelect(value: ValidIndexTarget): "first";
export declare function validIndexSelect(value: Source): "same";
export declare function readonlySelect(value: ReadonlyTarget): "chosen";
export declare function readonlySelect(value: never): "fallback";
