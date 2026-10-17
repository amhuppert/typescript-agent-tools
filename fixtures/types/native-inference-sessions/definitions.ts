export interface Box<T> { item: T; }
export interface Recursive<T> { next: T; }
export interface Node { next: Node; }
export interface parameterPrioritySource { <T>(value: T): T; }
export interface parameterPriorityTarget { (value: string): string | number; }
export declare const parameterPrioritySourceValue: parameterPrioritySource;
export declare function parameterPrioritySelect(value: parameterPriorityTarget): "chosen";
export declare function parameterPrioritySelect(value: parameterPrioritySource): "same";
export interface returnCombinationSource { <T>(): T; }
export interface returnCombinationTarget { (): string | number; }
export declare const returnCombinationSourceValue: returnCombinationSource;
export declare function returnCombinationSelect(value: returnCombinationTarget): "chosen";
export declare function returnCombinationSelect(value: returnCombinationSource): "same";
export interface returnConstraintFilterSource { <T extends string>(): T; }
export interface returnConstraintFilterTarget { (): string | number; }
export declare const returnConstraintFilterSourceValue: returnConstraintFilterSource;
export declare function returnConstraintFilterSelect(value: returnConstraintFilterTarget): "chosen";
export declare function returnConstraintFilterSelect(value: returnConstraintFilterSource): "same";
export interface constraintRejectSource { <T extends string>(value: T): T; }
export interface constraintRejectTarget { (value: number): number; }
export declare const constraintRejectSourceValue: constraintRejectSource;
export declare function constraintRejectSelect(value: constraintRejectTarget): "chosen";
export declare function constraintRejectSelect(value: constraintRejectSource): "same";
export interface dependentDefaultSource { <T = string, U extends T = T>(value: U): U; }
export interface dependentDefaultTarget { (value: number): number; }
export declare const dependentDefaultSourceValue: dependentDefaultSource;
export declare function dependentDefaultSelect(value: dependentDefaultTarget): "chosen";
export declare function dependentDefaultSelect(value: dependentDefaultSource): "same";
export interface forwardDefaultSource { <T = U, U = string>(value?: T): void; }
export interface forwardDefaultTarget { (): void; }
export declare const forwardDefaultSourceValue: forwardDefaultSource;
export declare function forwardDefaultSelect(value: forwardDefaultTarget): "chosen";
export declare function forwardDefaultSelect(value: forwardDefaultSource): "same";
export interface unusedDefaultSource { <T = MissingUnusedDefault>(value: T): T; }
export interface unusedDefaultTarget { (value: number): number; }
export declare const unusedDefaultSourceValue: unusedDefaultSource;
export declare function unusedDefaultSelect(value: unusedDefaultTarget): "chosen";
export declare function unusedDefaultSelect(value: unusedDefaultSource): "same";
export interface neededDefaultSource { <T = MissingNeededDefault, U extends T = T>(value: U): U; }
export interface neededDefaultTarget { (value: number): number; }
export declare const neededDefaultSourceValue: neededDefaultSource;
export declare function neededDefaultSelect(value: neededDefaultTarget): "chosen";
export declare function neededDefaultSelect(value: neededDefaultSource): "same";
export interface missingConstraintSource { <T extends MissingInferenceConstraint>(value: T): T; }
export interface missingConstraintTarget { (value: number): number; }
export declare const missingConstraintSourceValue: missingConstraintSource;
export declare function missingConstraintSelect(value: missingConstraintTarget): "chosen";
export declare function missingConstraintSelect(value: missingConstraintSource): "same";
export interface primitiveLiteralSource { <T extends string>(value: T): T; }
export interface primitiveLiteralTarget { (value: "a"): "a"; }
export declare const primitiveLiteralSourceValue: primitiveLiteralSource;
export declare function primitiveLiteralSelect(value: primitiveLiteralTarget): "chosen";
export declare function primitiveLiteralSelect(value: primitiveLiteralSource): "same";
export interface constLiteralSource { <const T>(value: T): T; }
export interface constLiteralTarget { (value: "a"): "a"; }
export declare const constLiteralSourceValue: constLiteralSource;
export declare function constLiteralSelect(value: constLiteralTarget): "chosen";
export declare function constLiteralSelect(value: constLiteralSource): "same";
export interface nestedObjectSource { <T>(value: { item: T }): T; }
export interface nestedObjectTarget { (value: { item: number }): number; }
export declare const nestedObjectSourceValue: nestedObjectSource;
export declare function nestedObjectSelect(value: nestedObjectTarget): "chosen";
export declare function nestedObjectSelect(value: nestedObjectSource): "same";
export interface referenceArgumentsSource { <T>(value: Box<T>): T; }
export interface referenceArgumentsTarget { (value: Box<number>): number; }
export declare const referenceArgumentsSourceValue: referenceArgumentsSource;
export declare function referenceArgumentsSelect(value: referenceArgumentsTarget): "chosen";
export declare function referenceArgumentsSelect(value: referenceArgumentsSource): "same";
export interface callbackInferenceSource { <T>(callback: (value: T) => void): T; }
export interface callbackInferenceTarget { (callback: (value: number) => void): number; }
export declare const callbackInferenceSourceValue: callbackInferenceSource;
export declare function callbackInferenceSelect(value: callbackInferenceTarget): "chosen";
export declare function callbackInferenceSelect(value: callbackInferenceSource): "same";
export interface candidateConflictSource { <T>(callback: (value: T) => void, value: T): T; }
export interface candidateConflictTarget { (callback: (value: string) => void, value: number): string | number; }
export declare const candidateConflictSourceValue: candidateConflictSource;
export declare function candidateConflictSelect(value: candidateConflictTarget): "chosen";
export declare function candidateConflictSelect(value: candidateConflictSource): "same";
export interface unionMatchingSource { <T>(value: T | string): T; }
export interface unionMatchingTarget { (value: number | string): number; }
export declare const unionMatchingSourceValue: unionMatchingSource;
export declare function unionMatchingSelect(value: unionMatchingTarget): "chosen";
export declare function unionMatchingSelect(value: unionMatchingSource): "same";
export interface recursiveConstraintSource { <T extends Recursive<T>>(value: T): T; }
export interface recursiveConstraintTarget { (value: Node): Node; }
export declare const recursiveConstraintSourceValue: recursiveConstraintSource;
export declare function recursiveConstraintSelect(value: recursiveConstraintTarget): "chosen";
export declare function recursiveConstraintSelect(value: recursiveConstraintSource): "same";
