export {};
declare const input: unknown;
const /*#checkedGood*/checkedGood = input satisfies { [P in 'a' | 'b']: number };
const /*#checkedName*/checkedName = input satisfies { [P in 'a' as Missing_name]: number };
const /*#checkedTemplate*/checkedTemplate = input satisfies { [P in 'a']: Missing_checked_template };
const /*#checkedConstraint*/checkedConstraint = input satisfies { [P in Missing_constraint]: number };
const /*#checkedInvalidKey*/checkedInvalidKey = input satisfies { [P in boolean]: number };
const /*#checkedInvalidName*/checkedInvalidName = input satisfies { [P in 'a' as boolean]: number };
const /*#quickUnchecked*/quickUnchecked = input as { [P in 'a' as Missing_unchecked]: number };
