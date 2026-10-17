interface Box {
  required: number;
  optional?: string;
  readonly fixed: 42;
  "\u0064ecoded": boolean;
  "": bigint;
  latent: Missing;
}
declare const box: Box;
declare const nullable: Box | undefined;
declare const anyValue: any;
declare const stringKey: "required";
declare const unionKey: "required" | "fixed";
interface Indexes { [key: string]: string | number; [key: number]: number; }
declare const indexes: Indexes;
interface Numeric { [key: number]: number; }
declare const numeric: Numeric;
export const dot = box.required;
export const literal = box["required"];
export const template = box[`required`];
export const escaped = box["\u0072equired"];
export const empty = box[""];
export const decoded = box.decoded;
export const identifier = box[stringKey];
export const union = box[unionKey];
export const fixed = box.fixed;
export const optional = box.optional;
export const optionalElement = box["optional"];
export const chain = nullable?.required;
export const elementChain = nullable?.["required"];
export const chainKnown = box?.fixed;
export const chainAny = anyValue?.required;
export const anyDot = anyValue.required;
export const anyElement = anyValue["required"];
export const stringIndex = indexes.word;
export const stringElement = indexes["word"];
export const numberIndex = indexes[2];
export const numericName = indexes["2"];
export const nonnumericName = indexes["02"];
export const specialNumericName = numeric["NaN"];
export const missingProperty = box.missing;
export const missingElement = box["missing"];
export const invalidReceiver = unknownReceiver.required;
export const invalidIndex = anyValue[unknownIndex];
export const visitedLatent = box.latent;
