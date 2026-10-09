// Types, enums, namespaces, and merged declarations.
export type Alias<T> = { value: T; list(): T[] };
type Union = 'a' | 'b';

export const enum Color {
  Red,
  Green = 2,
  'Light-Blue' = 3,
}

interface Merged { first: string }
interface Merged { second: number }

namespace NS {
  export const value = 1;
}
namespace NS {
  export function helper() {}
}

export namespace A.B.C {
  export const deep = 1;
  namespace Inner {
    export type T = string;
  }
}

class Both {}
namespace Both {
  export const extra = 1;
}

function fnNs() {}
namespace fnNs {
  export const attached = 1;
}

declare module "ambient-mod" {
  export function ambient(): void;
}

declare global {
  interface Window {
    navigation: string;
  }
}
