declare const other: boolean;
declare function classify(fn: (value: unknown) => value is string): "predicate";
declare function classify(fn: (value: unknown) => boolean): "boolean";
declare function classify(fn: unknown): "other";
function nonBoolean(value: unknown) { return 1; }
function multiple(value: unknown) { if (other) return true; return false; }
function bare(value: unknown) { return; }
function implicit(value: unknown) { if (other) return true; }
function unchanged(value: unknown) { return other; }
function booleanParameter(value: boolean) { return value; }
function assigned(value: unknown) { value = value; return other; }
function rest(...value: unknown[]) { return other; }
function inferred(value: unknown) { return typeof value === "string"; }
function oneSided(value: unknown) { return typeof value === "string" && other; }
const /*#nonBoolean*/nonBooleanResult = classify(nonBoolean);
const /*#multiple*/multipleResult = classify(multiple);
const /*#bare*/bareResult = classify(bare);
const /*#implicit*/implicitResult = classify(implicit);
const /*#unchanged*/unchangedResult = classify(unchanged);
const /*#booleanParameter*/booleanResult = classify(booleanParameter);
const /*#assigned*/assignedResult = classify(assigned);
const /*#rest*/restResult = classify(rest);
const /*#inferred*/inferredResult = classify(inferred);
const /*#oneSided*/oneSidedResult = classify(oneSided);
