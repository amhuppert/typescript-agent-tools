export {};
declare function targetBoth<T>(value: T extends string ? MissingTargetTrue : MissingTargetFalse): 11;
declare function targetBoth<T>(value: unknown): 12;
function targetBothUse<T>() {
    const /*#targetBoth*/selected = targetBoth<T>(0);
}
declare function targetTrue<T>(value: T extends unknown ? MissingTargetSelected : MissingTargetUnselected): 13;
declare function targetTrue<T>(value: unknown): 14;
function targetTrueUse<T>() {
    const /*#targetTrue*/selected = targetTrue<T>(0);
}
type SourceBranches<T> = T extends string ? MissingSourceTrue : MissingSourceFalse;
declare function sourceBoth<T>(value: { tag: 0 }): 21;
declare function sourceBoth<T>(value: unknown): 22;
function sourceBothUse<T>(value: SourceBranches<T>) {
    const /*#sourceBoth*/selected = sourceBoth<T>(value);
}
type ComparedSource<T> = T extends string ? MissingComparedSourceTrue : MissingComparedSourceFalse;
type ComparedTarget<T> = T extends string ? MissingComparedTargetTrue : MissingComparedTargetFalse;
declare function compareBranches<T>(value: ComparedTarget<T>): 23;
declare function compareBranches<T>(value: unknown): 24;
function compareBranchesUse<T>(value: ComparedSource<T>) {
    const /*#sourceTarget*/selected = compareBranches<T>(value);
}
declare function inferBranches<T, U>(value: T extends string ? [U, MissingInferenceTrue] : [U, MissingInferenceFalse]): U;
function inferBranchesUse<T>(value: T extends string ? [31, 0] : [32, 0]) {
    const /*#inferenceBoth*/selected = inferBranches(value);
}

declare const aliasKey: "Alias";
declare class Mixed { static [aliasKey]: 81; }
declare namespace Mixed { type Alias<T> = { value: T }; }
declare const createdSource: Mixed./*@createdAliasSymbol*/Alias<31>;
declare function createdRelation(value: Mixed.Alias<number>): 41;
declare function createdRelation(value: unknown): 42;
declare function createdInference<T>(value: Mixed.Alias<T>): T;
const /*#createdAliasValue*/createdValue = Mixed.Alias;
const /*#createdAliasRelation*/createdRelated = createdRelation(createdSource);
const /*#createdAliasInference*/createdInferred = createdInference(createdSource);

type Ordinary<T> = { value: T };
declare const ordinarySource: Ordinary<31>;
declare function ordinaryRelation(value: Ordinary<number>): 41;
declare function ordinaryRelation(value: unknown): 42;
declare function ordinaryInference<T>(value: Ordinary<T>): T;
const /*#ordinaryAliasRelation*/ordinaryRelated = ordinaryRelation(ordinarySource);
const /*#ordinaryAliasInference*/ordinaryInferred = ordinaryInference(ordinarySource);
const /*#healthy*/healthy = 45 as const;
