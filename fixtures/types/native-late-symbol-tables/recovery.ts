export {};
declare const selfKey: typeof self.name;
interface Self { name: "selfMember"; [selfKey]: 41; }
declare const self: Self;
const /*#recursiveEarlyTable*/recursiveEarlyTable = self.selfMember;

declare const emptySelfKey: keyof EmptySelf;
interface EmptySelf { [emptySelfKey]: 42; }
declare const emptySelf: EmptySelf;
const /*#recursiveNilTable*/recursiveNilTable = emptySelf.absent;

interface MissingName { [notDeclared]: 43; stable: 44; }
declare const missingName: MissingName;
const /*#nameRecovery*/nameRecovery = missingName.stable;

declare const valueKey: "value";
interface MissingValue { [valueKey]: MissingAnnotation; stable: 45; }
declare const missingValue: MissingValue;
const /*#valueRecoveryUntouched*/valueRecoveryUntouched = missingValue.stable;
const /*#valueRecoveryDemanded*/valueRecoveryDemanded = missingValue.value;

declare const missingKey: MissingKeyType;
interface MissingKey { [missingKey]: MissingSkippedAnnotation; stable: 46; }
declare const missingKeyValue: MissingKey;
const /*#nameTypeRecovery*/nameTypeRecovery = missingKeyValue.stable;
