import {
  returnVoidSource, returnVoidSelect,
  returnAnySource, returnAnySelect,
  returnUnknownSource, returnUnknownSelect,
  returnNumberSource, returnNumberSelect,
  targetReturnSource, targetReturnSelect,
  identitySource, identitySelect,
  arityEarlySource, arityEarlySelect,
  arityTailSource, arityTailSelect,
  extraTargetSource, extraTargetSelect,
  thisEarlySource, thisEarlySelect,
  requiredVoidSource, requiredVoidSelect,
  requiredUndefinedSource, requiredUndefinedSelect,
  optionalSource, optionalSelect,
  propertyFirstSource, propertyFirstSelect,
  callFirstSource, callFirstSelect,
  constructFirstSource, constructFirstSelect,
  indexInventorySource, indexInventorySelect,
  constructVoidSource, constructVoidSelect,
  constructAnySource, constructAnySelect,
  constructUnknownSource, constructUnknownSelect,
  functionVarianceSource, functionVarianceSelect,
  methodVarianceSource, methodVarianceSelect,
  callbackVarianceSource, callbackVarianceSelect,
  overloadSecondSource, overloadSecondSelect,
  overloadAllSource, overloadAllSelect,
  overloadMismatchSource, overloadMismatchSelect,
  predicateSameSource, predicateSameSelect,
  predicateIndexSource, predicateIndexSelect,
  predicateBooleanSource, predicateBooleanSelect,
  predicateMissingSource, predicateMissingSelect,
  genericContextSource, genericContextSelect,
  genericEraseSource, genericEraseSelect,
  abstractConcreteSource, abstractConcreteSelect,
  concreteAbstractSource, concreteAbstractSelect
} from "./definitions";
export const returnVoidResult = returnVoidSelect(returnVoidSource);
export const returnAnyResult = returnAnySelect(returnAnySource);
export const returnUnknownResult = returnUnknownSelect(returnUnknownSource);
export const returnNumberResult = returnNumberSelect(returnNumberSource);
export const targetReturnResult = targetReturnSelect(targetReturnSource);
export const identityResult = identitySelect(identitySource);
export const arityEarlyResult = arityEarlySelect(arityEarlySource);
export const arityTailResult = arityTailSelect(arityTailSource);
export const extraTargetResult = extraTargetSelect(extraTargetSource);
export const thisEarlyResult = thisEarlySelect(thisEarlySource);
export const requiredVoidResult = requiredVoidSelect(requiredVoidSource);
export const requiredUndefinedResult = requiredUndefinedSelect(requiredUndefinedSource);
export const optionalResult = optionalSelect(optionalSource);
export const propertyFirstResult = propertyFirstSelect(propertyFirstSource);
export const callFirstResult = callFirstSelect(callFirstSource);
export const constructFirstResult = constructFirstSelect(constructFirstSource);
export const indexInventoryResult = indexInventorySelect(indexInventorySource);
export const constructVoidResult = constructVoidSelect(constructVoidSource);
export const constructAnyResult = constructAnySelect(constructAnySource);
export const constructUnknownResult = constructUnknownSelect(constructUnknownSource);
export const functionVarianceResult = functionVarianceSelect(functionVarianceSource);
export const methodVarianceResult = methodVarianceSelect(methodVarianceSource);
export const callbackVarianceResult = callbackVarianceSelect(callbackVarianceSource);
export const overloadSecondResult = overloadSecondSelect(overloadSecondSource);
export const overloadAllResult = overloadAllSelect(overloadAllSource);
export const overloadMismatchResult = overloadMismatchSelect(overloadMismatchSource);
export const predicateSameResult = predicateSameSelect(predicateSameSource);
export const predicateIndexResult = predicateIndexSelect(predicateIndexSource);
export const predicateBooleanResult = predicateBooleanSelect(predicateBooleanSource);
export const predicateMissingResult = predicateMissingSelect(predicateMissingSource);
export const genericContextResult = genericContextSelect(genericContextSource);
export const genericEraseResult = genericEraseSelect(genericEraseSource);
export const abstractConcreteResult = abstractConcreteSelect(abstractConcreteSource);
export const concreteAbstractResult = concreteAbstractSelect(concreteAbstractSource);
