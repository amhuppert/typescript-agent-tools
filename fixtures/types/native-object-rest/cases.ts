declare const recordSource: { drop: 1; readonly kept: string; optional?: number; method(): boolean; get pair(): number; set pair(value: string); set setter(value: number); };
const { drop: droppedRecord, .../*#record*/recordRest } = recordSource;
declare class ClassSource { private secret: Missing_private; protected hidden: Missing_protected; data: string; readonly copied: number; method(): Missing_method; get accessor(): Missing_accessor; set onlySetter(value: Missing_setter); }
declare const instance: ClassSource;
const { .../*#class*/classRest } = instance;
declare const unionSource: { drop: 0; left: "left" } | { drop: 1; right: "right" };
const { drop: droppedUnion, .../*#union*/unionRest } = unionSource;
declare const nullableSource: { keep: number } | null | undefined;
const { .../*#nullable*/nullableRest } = nullableSource;
declare const anySource: any;
const { .../*#any*/anyRest } = anySource;
declare const emptySource: {};
const { .../*#empty*/emptyRest } = emptySource;
declare const indexedSource: { drop: 0; readonly [key: string]: number };
const { drop: droppedIndex, .../*#index*/indexRest } = indexedSource;
declare const excludedSource: { broken: Missing_excluded; kept: number };
const { broken: excluded, .../*#excluded*/excludedRest } = excludedSource;
declare const copiedFailureSource: { readonly broken: Missing_copied };
const { .../*#copiedFailure*/copiedFailure } = copiedFailureSource;
declare const unknownSource: unknown;
const { .../*#unknown*/unknownRest } = unknownSource;
declare const numberSource: number;
const { .../*#number*/numberRest } = numberSource;
declare const falsySource: { keep: number } | false | 0 | "";
const { .../*#falsy*/falsyRest } = falsySource;
declare const truthySource: { keep: number } | true;
const { .../*#truthy*/truthyRest } = truthySource;
function retain<T extends object>(value: T) { const { .../*#noKeys*/noKeys } = value; return noKeys; }
function remove<T extends { drop: number; kept: string }>(value: T) { const { drop: droppedGeneric, .../*#generic*/genericRest } = value; return genericRest; }
function remaining<T>(value: { drop: number; kept: T }) { const { drop: droppedParameter, .../*#parameter*/parameterRest } = value; return parameterRest; }
declare const concrete: { drop: number; kept: string };
const /*#instantiated*/instantiatedRest = remaining<string>(concrete);
let assignedRest: { kept: string };
let assignedDrop: number;
const /*#assignment*/assignmentRest = ({ drop: assignedDrop, ...assignedRest } = concrete);
