// A declaration file: ambient modules, global augmentation, overloads.
declare module "pkg" {
  export interface Options {
    verbose?: boolean;
  }
  export function run(options: Options): void;
  export function run(name: string, options?: Options): void;
  global {
    var pkgVersion: string;
  }
}

declare module "pkg/*";

declare namespace Lib.Nested {
  function nestedCall(): void;
  let counter: number;
}

declare global {
  interface Array<T> {
    customFind(predicate: (value: T) => boolean): T | undefined;
  }
}

export {};
