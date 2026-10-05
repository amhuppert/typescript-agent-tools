// The design's alias example: value flow through `copy` is not an alias.
export function /*@run:de*/run() {}
const /*@copy:d*/copy = /*@run:u*/run;
/*@copy:u*/copy();
