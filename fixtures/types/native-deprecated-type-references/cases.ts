export {};
/** @deprecated */
type Old = number;
/** @deprecated */
type OldGeneric<T> = number;
/** @deprecated */
type Constrained<T extends string> = number;
/** Ordinary documentation. */
type Current = number;
/** Text mentions deprecated without a tag. */
type HintOnly = number;
interface Joined { value?: number }
/** @deprecated */
interface Joined { other?: string }
/** @deprecated */
interface FirstDeprecated { value?: number }
interface FirstDeprecated { other?: string }
const /*#quick*/quick = 0 as Old;
const /*#checked*/checked = (0 as Old) satisfies number;
const /*#quickGeneric*/quickGeneric = 0 as OldGeneric<number>;
const /*#checkedGeneric*/checkedGeneric = (0 as OldGeneric<number>) satisfies number;
const /*#rejectedConstraint*/rejectedConstraint = (0 as Constrained<number>) satisfies number;
const /*#badArity*/badArity = (0 as Old<number>) satisfies number;
const /*#missing*/missing = (0 as MissingReference) satisfies number;
const /*#ordinary*/ordinary = (0 as Current) satisfies number;
const /*#hintOnly*/hintOnly = (0 as HintOnly) satisfies number;
const /*#mergedLater*/mergedLater = ({} as Joined) satisfies Joined;
const /*#mergedFirst*/mergedFirst = ({} as FirstDeprecated) satisfies FirstDeprecated;
const /*#healthyAfter*/healthyAfter = 1 + 2;
