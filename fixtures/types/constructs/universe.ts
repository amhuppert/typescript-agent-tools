import { Bag, Empty, Numbered, Options, Pair, View } from './definitions';

declare const fullView: View;
declare const dynamicKey: string;

// Each literal's named members name only keys its target does not declare:
// not examined for it by default. All but `pairOrView` are TS2353 errors.
export const wrongKeys: View = { other: 1 };
export const pairOrView: Pair | View = { id: 'q', status: 'open' };
export const spreadOther: View = { ...fullView, other: 1 };
export const lateOther: Options = { [dynamicKey]: 'x', other: 1 };
// With no named member, a spread or a late-bound name may write any key.
export const unnamed: Options = { ...fullView, [dynamicKey]: 'x' };
// An index signature admits every key.
export const bag: Bag = { apples: 1 };
// The binder's name decides: `0x1` writes `1`.
export const numbered: Numbered = { 0x1: 'one' };
// A type assertion relates comparability, without excess properties: valid,
// and not examined by default.
export const assertedOther = { other: 1 } as Options;
// A signed numeric name is the binder's: `[-1]` names `-1`.
export const signed = { [-1]: 'm', other: 1 } as Numbered;
// An empty target has no excess-property check: every literal is examined.
export const emptyTarget: Empty = { meaningful: 1 };
