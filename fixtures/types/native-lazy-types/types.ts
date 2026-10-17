export interface Lazy {
  known: string;
  deferred: string extends number ? string : number;
}
export interface Broken {
  known: number;
  missing: MissingType;
}
export declare const lazy: Lazy;
export declare const broken: Broken;
export type DeferredAlias = string extends number ? string : number;
export interface Repeated {
  first: DeferredAlias;
  second: DeferredAlias;
}
