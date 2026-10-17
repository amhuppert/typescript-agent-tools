interface Array<T> { length: number; [index: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }
type HiddenTuple = [MissingHidden];
type GenericTuple<T> = [T, MissingGeneric];
type KnownTuple = [string];
declare const /*#hidden*/hidden: HiddenTuple;
declare const /*#generic*/generic: GenericTuple<number>;
declare const /*#known*/known: KnownTuple;
