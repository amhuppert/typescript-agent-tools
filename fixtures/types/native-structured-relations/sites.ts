import { source, optionalSource, sourceIndex, unknownSelect, anySelect, numberSelect,
  mismatchFirstSelect, mismatchLastSelect, identitySelect, unmatchedSelect,
  optionalUnknownSelect, optionalRequiredSelect, sourceIndexSelect, targetIndexSelect,
  validIndexSelect, readonlySelect } from "./definitions";
export const unknownResult = unknownSelect(source);
export const anyResult = anySelect(source);
export const numberResult = numberSelect(source);
export const mismatchFirstResult = mismatchFirstSelect(source);
export const mismatchLastResult = mismatchLastSelect(source);
export const identityResult = identitySelect(source);
export const unmatchedResult = unmatchedSelect(source);
export const optionalUnknownResult = optionalUnknownSelect(optionalSource);
export const optionalRequiredResult = optionalRequiredSelect(optionalSource);
export const sourceIndexResult = sourceIndexSelect(sourceIndex);
export const targetIndexResult = targetIndexSelect(source);
export const validIndexResult = validIndexSelect(source);
export const readonlyResult = readonlySelect(source);
