export {};
declare const stringIndex: string;
declare const numberIndex: number;
declare const symbolIndex: symbol;
declare const uniqueIndex: unique symbol;
declare const unionIndex: string | number;
declare const nullableIndex: string | undefined;
declare const rejectedIndex: boolean;
interface StringIndex { [stringIndex]: 31; }
interface NumberIndex { [numberIndex]: 32; }
interface SymbolIndex { [symbolIndex]: 33; }
interface UnionIndex { [unionIndex]: 34; }
interface NullableIndex { [nullableIndex]: 35; stable: 36; }
interface RejectedIndex { [rejectedIndex]: MissingRejectedValue; stable: 37; }
interface ExplicitIndex { [key: string]: 38; [stringIndex]: MissingOverriddenValue; }
interface CombinedIndex { [stringIndex]: 39; own: 40; }
declare const strings: StringIndex;
declare const numbers: NumberIndex;
declare const symbols: SymbolIndex;
declare const unions: UnionIndex;
declare const nullables: NullableIndex;
declare const rejected: RejectedIndex;
declare const explicit: ExplicitIndex;
declare const combined: CombinedIndex;
const /*#stringIndexFallback*/stringIndexFallback = strings.unknown;
const /*#numberIndexFallback*/numberIndexFallback = numbers[100];
const /*#symbolIndexFallback*/symbolIndexFallback = symbols[uniqueIndex];
const /*#unionIndexFallback*/unionIndexFallback = unions.unknown;
const /*#nullableIndexOwn*/nullableIndexOwn = nullables.stable;
const /*#nullableIndexFallback*/nullableIndexFallback = nullables.unknown;
const /*#rejectedIndexUntouched*/rejectedIndexUntouched = rejected.stable;
const /*#explicitIndexWins*/explicitIndexWins = explicit.unknown;
const /*#indexIncludesSibling*/indexIncludesSibling = combined.unknown;
