// A script merges an interface into the library's global one.
interface /*@window:d*/Window { /*@!*/custom: number; }
declare var /*@frame:d*/frame: /*@window*/Window;
