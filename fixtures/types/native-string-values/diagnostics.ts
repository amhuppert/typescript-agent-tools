export {};
namespace High { export enum E { Member = "\uD800" } }
namespace Same { export enum E { Member = "\uD800" } }
namespace Low { export enum E { Member = "\uDC00" } }
namespace Concatenated { export enum E { Member = "\uD800" + "\uDC00" } }
namespace Paired { export enum E { Member = "\uD800\uDC00" } }
declare function acceptHigh(value: High.E): void;
declare function acceptLow(value: Low.E): void;
declare function acceptPaired(value: Paired.E): void;
declare const high: High.E;
declare const same: Same.E;
declare const low: Low.E;
declare const concatenated: Concatenated.E;
acceptHigh(/*#enumEqual*/same);
acceptHigh(/*#enumHighLow*/low);
acceptLow(/*#enumLowHigh*/high);
acceptPaired(/*#enumConcatPair*/concatenated);
