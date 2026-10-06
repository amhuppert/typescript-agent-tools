// Names are decoded: escapes, astral characters, and leading underscores.
const /*@escaped:d*/\u0061bc = 1;
/*@escaped:u*/abc;
/*@escaped:u*/\u{61}bc;
const /*@astral:d*/𝑥 = "😀"; /*@astral:u*/𝑥;
/*@astral:u*/\u{1D465};
const /*@underscore2:d*/__proto = 2;
/*@underscore2:u*/__proto;
const /*@underscore3:d*/___triple = 3;
/*@underscore3:u*/___triple;
const /*@underscore1:d*/_single = 4; /*@underscore1:u*/_single;
const /*@dollar:d*/$dollar = 5; /*@dollar:u*/$dollar;
const /*@accent:d*/café = 6; /*@accent:u*/caf\u00e9;
export {};
