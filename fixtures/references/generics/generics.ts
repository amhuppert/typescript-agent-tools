// Generic parameter scopes, mapped types, and conditional `infer` bindings,
// which are visible only in the true branch.
type /*@outerT*/T = boolean;
type /*@Element*/Element = string;

function /*@identity*/identity</*@fnT*/T>(/*@value*/value: /*@fnT*/T): /*@fnT*/T { return /*@value*/value; }

class /*@Container*/Container</*@classT*/T> {
  /*@!*/item!: /*@classT*/T;
  /*@!*/map</*@U*/U>(/*@mapper*/mapper: (/*@mapped*/value: /*@classT*/T) => /*@U*/U): /*@U*/U {
    return /*@mapper*/mapper(this./*@!*/item);
  }
}

interface /*@Wrapper*/Wrapper</*@wrapperV*/V = /*@outerT*/T> { /*@!*/value: /*@wrapperV*/V }

type /*@Mapped*/Mapped</*@mappedT*/T> = { readonly [/*@P*/P in keyof /*@mappedT*/T]: /*@mappedT*/T[/*@P*/P] };
type /*@Renamed*/Renamed</*@renamedT*/T> = { [/*@K*/K in keyof /*@renamedT*/T as /*@K*/K]: /*@Element*/Element };

type /*@Unwrap*/Unwrap</*@unwrapT*/T> = /*@unwrapT*/T extends /*@Wrapper*/Wrapper<infer /*@inferred*/Element> ? /*@inferred*/Element : /*@Element*/Element;

function /*@pick*/pick</*@objT*/T extends /*@Wrapper*/Wrapper, /*@KeyT*/K extends keyof /*@objT*/T>(/*@object*/object: /*@objT*/T, /*@key*/key: /*@KeyT*/K) {
  return /*@object*/object[/*@key*/key];
}

let /*@t*/t: /*@outerT*/T = /*@identity*/identity(true);
let /*@m*/m: /*@Mapped*/Mapped</*@Wrapper*/Wrapper> | /*@Renamed*/Renamed</*@Wrapper*/Wrapper> | /*@Unwrap*/Unwrap</*@Wrapper*/Wrapper> = null!;
new /*@Container*/Container<number>();
/*@pick*/pick; /*@t*/t; /*@m*/m;
export {};
