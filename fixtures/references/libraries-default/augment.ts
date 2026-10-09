// A module augments the library's `Array` interface globally.
declare /*@!*/global {
  interface /*@array:d*/Array</*@item:d*/T> {
    /*@!*/last(): /*@item*/T | undefined;
  }
}
export const /*@list:de*/list: /*@array*/Array<number> = [];
/*@list*/list./*@!*/last();
