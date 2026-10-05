// `{value}` uses the lexical value; `{value: other}` uses `other`. Assignment
// patterns write through the same names.
let /*@alpha*/alpha = 1;
let /*@beta*/beta = 2;
const /*@object*/object = { /*@alpha*/alpha, /*@!*/renamed: /*@beta*/beta, /*@!*/method() { return /*@alpha*/alpha; } };
const /*@copy*/copy = { .../*@object*/object, /*@beta*/beta };

({ /*@alpha*/alpha } = /*@object*/object);
({ /*@!*/renamed: /*@beta*/beta } = /*@object*/object);
({ /*@alpha*/alpha = /*@beta*/beta } = /*@object*/object);
[/*@alpha*/alpha, /*@beta*/beta] = [/*@beta*/beta, /*@alpha*/alpha];

function /*@shadow*/shadow(/*@param*/alpha: number) {
  return { /*@param*/alpha };
}
/*@shadow*/shadow(/*@copy*/copy./*@!*/alpha);
export {};
