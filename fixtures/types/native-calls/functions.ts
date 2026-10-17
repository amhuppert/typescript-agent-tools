export function fixed(): number { return Missing; }
export function inferred() { return 1; }
export function literal(): "ready" { return Missing; }
export function optionalUnused(value?: Missing): number { return Missing; }
export function voidOmitted(value: void): number { return Missing; }
export function required(value: number): number { return Missing; }
export function missingRequired(value: Missing): number { return Missing; }
export function withVoidThis(this: void): number { return Missing; }
export function withObjectThis(this: { tag: string }): number { return Missing; }
export function overloaded(value: "ready"): "text";
export function overloaded(value: number): "number";
export function overloaded(value: any): any { return Missing; }
export function generic<T>(value: T): T { return value; }
export declare const anyTarget: any;
export declare const unknownTarget: unknown;
export function mutualA() { return mutualB(); }
export function mutualB() { return mutualA(); }
export function self() { return self(); }
export function recursiveAnnotation(): typeof recursiveAnnotation { return Missing; }
export function predicate(value: unknown): value is Missing { return Missing; }
export const fixedResult = fixed();
export const inferredResult = inferred();
export const literalResult = literal();
export const optionalResult = optionalUnused();
export const voidOmittedResult = voidOmitted();
export const voidThisResult = withVoidThis();
export const requiredMissingResult = required();
export const tooManyResult = fixed(1);
export const tooManyMissingResult = fixed(Missing);
export const wrongArgumentResult = required("wrong");
export const wrongThisResult = withObjectThis();
export const missingRequiredTypeResult = missingRequired();
export const untypedResult = anyTarget(1, "ready");
export const untypedZeroResult = anyTarget();
export const untypedMissingResult = anyTarget(Missing);
export const unknownResult = unknownTarget();
export const missingCalleeResult = Missing();
export const overloadedResult = overloaded("ready");
export const overloadFailureResult = overloaded(true);
export const genericResult = generic("ready");
export const explicitGenericResult = generic<number>(1);
export const badTypeArgumentsResult = generic<number, string>(1);
export const mutualResult = mutualA();
export const directSelfResult = self();
export const recursiveAnnotationResult = recursiveAnnotation();
export const predicateResult = predicate(1);
// Inferred return bodies check call expressions, unlike the single-signature
// quick path used by ordinary variable initializers above.
export function checkedFixed() { return fixed(); }
export function checkedInferred() { return inferred(); }
export function checkedLiteral() { return literal(); }
export function checkedOptional() { return optionalUnused(); }
export function checkedVoidOmitted() { return voidOmitted(); }
export function checkedVoidThis() { return withVoidThis(); }
export function checkedRequiredMissing() { return required(); }
export function checkedTooManyMissing() { return fixed(Missing); }
export function checkedMissingRequiredType() { return missingRequired(); }
export function checkedUntypedZero() { return anyTarget(); }
export function checkedMissingCallee() { return Missing(); }
