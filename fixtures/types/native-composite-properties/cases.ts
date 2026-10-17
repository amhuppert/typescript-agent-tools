export {};
interface A { p: number; a: boolean; }
interface B { p: string; b: boolean; }
interface Same { p: number; }
interface Optional { p?: number; }
interface ReadonlyA { readonly p: number; }
declare const union: A | B;
declare const intersection: A & B;
declare const same: A | Same;
declare const optional: A | Optional;
declare const required: A & Optional;
declare const readonlyUnion: A | ReadonlyA;
declare const writableIntersection: A & ReadonlyA;
const readUnion = /*#union*/union.p;
const readIntersection = /*#intersection*/intersection.p;
const readSame = /*#same*/same.p;
const readOptional = /*#optional*/optional.p;
const readRequired = /*#required*/required.p;
const readPartial = /*#partial*/union.a;
/*#readonlyWrite*/readonlyUnion.p = 1;
/*#intersectionWrite*/writableIntersection.p = 1;
interface ReadText { get p(): string; set p(value: string | boolean); }
interface ReadNumber { get p(): number; set p(value: number | boolean); }
declare const accessors: ReadText | ReadNumber;
const readAccessors = /*#accessorRead*/accessors.p;
/*#accessorWrite*/accessors.p = true;
interface Index { [key: string]: number; }
declare const indexed: A | Index;
const readIndexed = /*#indexed*/indexed.p;
interface Third { p: boolean; }
declare const deferred: A | B | Third;
const readDeferred = /*#deferred*/deferred.p;
interface Latent { p: Missing; }
declare const unhealthy: A | Latent;
const readUnhealthy = /*#unhealthy*/unhealthy.p;
function nullable(receiver: A | undefined): void {
 receiver?.p;
 receiver?.p;
 const restored = /*#restored*/receiver;
}
function discriminant(receiver: { p: 0; a: number } | { p: 1; b: number }): void {
 if (receiver.p) { const narrowed = /*#narrowed*/receiver; }
 const joined = /*#joined*/receiver;
}

declare const literalConflict: { p: 0; q: number } & { p: 1; q: number };
declare const optionalConflict: { p?: 0; q: number } & { p?: 1; q: number };
declare const originalNever: { p: never; q: number } & { p: 1; q: number };
declare const wideConflict: { p: string; q: number } & { p: number; q: number };
class PrivateLeft { private p: number; q: number; }
class PrivateRight { private p: number; q: number; }
declare const privateConflict: PrivateLeft & PrivateRight;
const literalConflictResult = /*#literalConflict*/literalConflict.q;
const optionalConflictResult = /*#optionalConflict*/optionalConflict.q;
const originalNeverResult = /*#originalNever*/originalNever.q;
const wideConflictResult = /*#wideConflict*/wideConflict.q;
const privateConflictResult = /*#privateConflict*/privateConflict.q;
