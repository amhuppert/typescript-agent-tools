export function annotated(this: { label: string }, required: string, optional?: number, initialized: boolean = true): number { return MissingReturn; }
export declare function declared(required: string, optional?: number): boolean;
export declare function bodyless(value: string);
export declare function invalid(value: MissingType): number;
export type CallType = (this: { label: string }, required: string, optional?: number) => boolean;
export type ConstructorType = new (required: string) => { value: number };
export interface Callable { (required: string, optional?: number): boolean; new (required: string): { value: number }; }
export declare function isText(value: unknown): value is string;
export declare function assertText(value: unknown): asserts value is string;
export declare function assertPresent(value: unknown): asserts value;
export declare function generic<T extends string = "ready">(value: T): T;
export function overloaded(value: string): string;
export function overloaded(value: number): number;
export function overloaded(value: unknown): unknown { return MissingReturn; }
export function defaultsBeforeRequired(first: number = 1, required: string, last: boolean = false): number { return MissingReturn; }
export function defaultBeforeVoid(first: number = 1, required: void): number { return MissingReturn; }
export function thisBeforeDefaults(this: string, first: number = 1, required: string, last: boolean = false): number { return MissingReturn; }
export function questionBeforeRequired(first?: number, required: string): number { return MissingReturn; }
export type Word = "ready";
export declare function literalAlias(value: Word): Word;
export function explicitUndefined(first: number | undefined = 1, required: string): number { return MissingReturn; }
export function inferredDefaultsBeforeRequired(first = 1, required: string): number { return MissingReturn; }
export function separated(value: string): string;
export const separation = true;
export function separated(value: number): number { return MissingReturn; }
export function commented(value: string): string;
// An intervening comment is trivia, so the implementation is still adjacent.
export function commented(value: unknown): unknown { return MissingReturn; }
export namespace Unrelated { export declare function overloaded(value: boolean): boolean; }
export namespace Joined { export function parentSeparated(value: string): string; }
export namespace Joined { export function parentSeparated(value: number): number { return MissingReturn; } }
export declare function isThis(this: unknown): this is string;
export declare function assertThis(this: unknown): asserts this is string;
export declare function predicateMissing(value: unknown): value is MissingType;
export interface PredicateContainer { (value: unknown): value is MissingType; }
export declare function plainGeneric<T>(value: T): T;
export declare function constGeneric<const T extends string = "ready">(value: T): T;
export declare function escapedGeneric<\u0055 extends number = 1>(value: \u0055): \u0055;
export type GenericCall = <T extends string = "ready">(value: T) => T;
export type GenericConstructor = new <T extends string = "ready">(value: T) => { value: T };
export interface GenericCallable { <T extends string = "ready">(value: T): T; }
export interface GenericMissing { <T extends MissingConstraint = MissingDefault>(value: T): T; }
export function genericOverloaded<T extends string>(value: T): T;
export function genericOverloaded<U extends number>(value: U): U;
export function genericOverloaded(value: unknown): unknown { return MissingReturn; }
export declare function genericAny<T extends any = any>(value: T): T;
export declare function genericInvalid<T extends MissingConstraint = MissingDefault>(value: T): T;
export declare function varianceIn<in T>(value: T): T;
export declare function varianceOut<out T>(value: T): T;
export declare function varianceBoth<in out T>(value: T): T;
export declare function constVariance<const in out T>(value: T): T;
export function sameGenericName<T extends string>(value: T): T;
export function sameGenericName<T extends number>(value: T): T;
export function sameGenericName(value: unknown): unknown { return MissingReturn; }
