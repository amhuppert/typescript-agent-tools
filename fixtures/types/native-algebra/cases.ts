interface Array<T> { length: number; [index: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }
interface A { alpha: string }
interface B { beta: number }
declare const /*#intersectionSame*/intersectionSame: string & string;
declare const /*#intersectionDisjoint*/intersectionDisjoint: string & number;
declare const /*#intersectionUnknown*/intersectionUnknown: unknown & string;
declare const /*#intersectionAny*/intersectionAny: any & number;
declare const /*#intersectionLiteral*/intersectionLiteral: string & 'ready';
declare const /*#intersectionUnits*/intersectionUnits: 'ready' & 'done';
declare const /*#intersectionBoolean*/intersectionBoolean: true & false;
declare const /*#intersectionEmpty*/intersectionEmpty: {} & unknown;
declare const /*#intersectionNullable*/intersectionNullable: A & null;
declare const /*#intersectionUndefined*/intersectionUndefined: A & undefined;
declare const /*#intersectionNullUnions*/intersectionNullUnions: (string | null) & (number | null);
declare const /*#intersectionUndefinedUnions*/intersectionUndefinedUnions: (string | undefined) & (number | undefined);
declare const /*#intersectionOrder*/intersectionOrder: B & A;
declare const /*#intersectionFlatten*/intersectionFlatten: (B & A) & (A & B);
declare const /*#intersectionMissing*/intersectionMissing: number & MissingIntersection;
declare const /*#intersectionProduct*/intersectionProduct: (A | { gamma: boolean }) & (B | { delta: bigint });
declare const /*#tupleEmpty*/tupleEmpty: [];
declare const /*#tupleFixed*/tupleFixed: [string, number];
declare const /*#tupleOptional*/tupleOptional: [string, number?];
declare const /*#tupleLabelled*/tupleLabelled: [title: string, count?: number];
declare const /*#tupleReadonly*/tupleReadonly: readonly [title: string, count?: number];
declare const /*#tupleRestOnly*/tupleRestOnly: [...number[]];
declare const /*#tupleReadonlyRestOnly*/tupleReadonlyRestOnly: readonly [...number[]];
declare const /*#tupleFixedRest*/tupleFixedRest: [string, ...number[]];
declare const /*#tupleMissing*/tupleMissing: [MissingTuple];
declare const /*#keyofObject*/keyofObject: keyof { a: string; b: number };
declare const /*#keyofUnion*/keyofUnion: keyof ({ shared: string; a: number } | { shared: number; b: boolean });
declare const /*#keyofIntersection*/keyofIntersection: keyof (A & B);
declare const /*#keyofUnknown*/keyofUnknown: keyof unknown;
declare const /*#keyofAny*/keyofAny: keyof any;
declare const /*#keyofNever*/keyofNever: keyof never;
declare const /*#indexedObject*/indexedObject: { value: number }['value'];
declare const /*#indexedOptional*/indexedOptional: { value?: string }['value'];
declare const /*#indexedMissing*/indexedMissing: { value: number }['missing'];
declare const /*#conditionalTrue*/conditionalTrue: string extends string ? number : boolean;
declare const /*#conditionalFalse*/conditionalFalse: string extends number ? number : boolean;
declare const /*#conditionalSkipped*/conditionalSkipped: string extends number ? MissingSkipped : number;
declare const /*#conditionalDemanded*/conditionalDemanded: string extends string ? MissingDemanded : number;
declare const /*#templateResolved*/templateResolved: `head-${'a' | 'b'}`;
function generic<T, K extends keyof T, V extends unknown[], S extends string>(): void {
 let /*#keyofGeneric*/keyofGeneric!: keyof T;
 let /*#indexedGeneric*/indexedGeneric!: T[K];
 let /*#conditionalGeneric*/conditionalGeneric!: T extends string ? number : boolean;
 let /*#mappedGeneric*/mappedGeneric!: { readonly [P in keyof T]?: T[P] };
 let /*#templateGeneric*/templateGeneric!: `head-${S}`;
 let /*#tupleVariadic*/tupleVariadic!: [string, ...V];
}
function constrained<T extends 'ready' | 'done', U extends string | number>(): void {
 let /*#intersectionConstrainedSame*/intersectionConstrainedSame!: T & string;
 let /*#intersectionConstrainedDisjoint*/intersectionConstrainedDisjoint!: T & number;
 let /*#intersectionConstrainedNarrow*/intersectionConstrainedNarrow!: U & 'ready';
}

type IntersectionAlias = B & A;
declare const /*#intersectionAlias*/intersectionAlias: IntersectionAlias;
type OtherIntersectionAlias = B & A;
declare const /*#intersectionOtherAlias*/intersectionOtherAlias: OtherIntersectionAlias;
declare const /*#tupleEscapedLabel*/tupleEscapedLabel: [\u0074itle: string];
declare const /*#intersectionAnySkipsIndex*/intersectionAnySkipsIndex: { [key: MissingIndexKey]: string } & any;
declare const /*#intersectionNeverSkipsIndex*/intersectionNeverSkipsIndex: { [key: MissingIndexKey]: string } & string & number;
declare const /*#intersectionAnySkipsReturn*/intersectionAnySkipsReturn: { (): MissingIntersectionReturn } & any;
declare const /*#intersectionNeverSkipsReturn*/intersectionNeverSkipsReturn: { (): MissingIntersectionReturn } & string & number;
declare const /*#intersectionLiteralEmptyString*/intersectionLiteralEmptyString: {} & string;
declare const /*#intersectionLiteralEmptyNumber*/intersectionLiteralEmptyNumber: number & {};
declare const /*#intersectionLiteralEmptyBigint*/intersectionLiteralEmptyBigint: {} & bigint;
type EmptyIntersectionAlias = {};
declare const /*#intersectionAliasedEmptyString*/intersectionAliasedEmptyString: EmptyIntersectionAlias & string;
