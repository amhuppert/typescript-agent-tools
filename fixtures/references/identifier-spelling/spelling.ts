// Names are decoded: escapes, astral characters, and leading underscores.
const /*@escaped*/\u0061bc = 1;
/*@escaped*/abc;
/*@escaped*/\u{61}bc;
const /*@astral*/𝑥 = "😀"; /*@astral*/𝑥;
/*@astral*/\u{1D465};
const /*@underscore2*/__proto = 2;
/*@underscore2*/__proto;
const /*@underscore3*/___triple = 3;
/*@underscore3*/___triple;
const /*@underscore1*/_single = 4; /*@underscore1*/_single;
const /*@dollar*/$dollar = 5; /*@dollar*/$dollar;
const /*@accent*/café = 6; /*@accent*/caf\u00e9;
export {};
