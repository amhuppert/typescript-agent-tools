interface LiteralChain<U extends "ready", T extends U> { literal: /*#literal*/T; }
interface FreeChain<U, T extends U> { free: /*#free*/T; }
interface LiteralUnion<U extends "left", V extends 7, T extends U | V> { either: /*#either*/T; }
interface PartialFree<U, V extends 7, T extends U | V> { partial: /*#partial*/T; }
interface ObjectChain<U extends { value: string }, T extends U> { object: /*#object*/T; }
interface Box<X> { value: X; }
interface Reference<T extends Box<number>> { reference: /*#reference*/T; }
interface Recursive<T extends { next: T }> { recursive: /*#recursive*/T; }
interface DirectCycle<T extends T> { direct: /*#direct*/T; }
interface MutualCycle<T extends U, U extends T> { mutual: /*#mutual*/T; }
interface Missing<T extends MissingType> { missing: /*#missing*/T; }
interface ThisRoot<T> { self: /*#this*/this; }
