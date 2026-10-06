// Generic parameter scopes, mapped types, and conditional `infer` bindings,
// which are visible only in the true branch.
type /*@outerT:d*/T = boolean;
type /*@Element:d*/Element = string;

function /*@identity:d*/identity</*@fnT:d*/T>(/*@value:d*/value: /*@fnT:u*/T): /*@fnT:u*/T { return /*@value:u*/value; }

class /*@Container:d*/Container</*@classT:d*/T> {
  /*@!*/item!: /*@classT:u*/T;
  /*@!*/map</*@U:d*/U>(/*@mapper:d*/mapper: (/*@mapped:d*/value: /*@classT:u*/T) => /*@U:u*/U): /*@U:u*/U {
    return /*@mapper:u*/mapper(this./*@!*/item);
  }
}

interface /*@Wrapper:d*/Wrapper</*@wrapperV:d*/V = /*@outerT:u*/T> { /*@!*/value: /*@wrapperV:u*/V }

type /*@Mapped:d*/Mapped</*@mappedT:d*/T> = { readonly [/*@P:d*/P in keyof /*@mappedT:u*/T]: /*@mappedT:u*/T[/*@P:u*/P] };
type /*@Renamed:d*/Renamed</*@renamedT:d*/T> = { [/*@K:d*/K in keyof /*@renamedT:u*/T as /*@K:u*/K]: /*@Element:u*/Element };

type /*@Unwrap:d*/Unwrap</*@unwrapT:d*/T> = /*@unwrapT:u*/T extends /*@Wrapper:u*/Wrapper<infer /*@inferred:d*/Element> ? /*@inferred:u*/Element : /*@Element:u*/Element;

function /*@pick:d*/pick</*@objT:d*/T extends /*@Wrapper:u*/Wrapper, /*@KeyT:d*/K extends keyof /*@objT:u*/T>(/*@object:d*/object: /*@objT:u*/T, /*@key:d*/key: /*@KeyT:u*/K) {
  return /*@object:u*/object[/*@key:u*/key];
}

let /*@t:d*/t: /*@outerT:u*/T = /*@identity:u*/identity(true);
let /*@m:d*/m: /*@Mapped:u*/Mapped</*@Wrapper:u*/Wrapper> | /*@Renamed:u*/Renamed</*@Wrapper:u*/Wrapper> | /*@Unwrap:u*/Unwrap</*@Wrapper:u*/Wrapper> = null!;
new /*@Container:u*/Container<number>();
/*@pick:u*/pick; /*@t:u*/t; /*@m:u*/m;
export {};

// An `infer` constraint sees its own declaration, but not a later sibling's.
type /*@SelfOuter:d*/Self = string;
type /*@SelfRef:d*/SelfRef</*@selfA:d*/A> = /*@selfA:u*/A extends infer /*@selfInner:d*/Self extends { /*@!*/self: /*@selfInner:u*/Self } ? /*@selfInner:u*/Self : /*@SelfOuter:u*/Self;
type /*@SiblingOuter:d*/Sibling = string;
type /*@Siblings:d*/Siblings</*@sibA:d*/A> = /*@sibA:u*/A extends [infer /*@sibU:d*/U extends { /*@!*/u: /*@sibU:u*/U; /*@!*/v: /*@SiblingOuter:u*/Sibling }, infer /*@sibInner:d*/Sibling] ? [/*@sibU:u*/U, /*@sibInner:u*/Sibling] : /*@SiblingOuter:u*/Sibling;
let /*@inferUses:d*/inferUses: [/*@SelfRef:u*/SelfRef<1>, /*@Siblings:u*/Siblings<[]>] = null!;
/*@inferUses:u*/inferUses;
