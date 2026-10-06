// `{value}` uses the lexical value; `{value: other}` uses `other`. Assignment
// patterns write through the same names.
let /*@alpha:d*/alpha = 1;
let /*@beta:d*/beta = 2;
const /*@object:d*/object = { /*@alpha:u*/alpha, /*@!*/renamed: /*@beta:u*/beta, /*@!*/method() { return /*@alpha:u*/alpha; } };
const /*@copy:d*/copy = { .../*@object:u*/object, /*@beta:u*/beta };

({ /*@alpha:u*/alpha } = /*@object:u*/object);
({ /*@!*/renamed: /*@beta:u*/beta } = /*@object:u*/object);
({ /*@alpha:u*/alpha = /*@beta:u*/beta } = /*@object:u*/object);
[/*@alpha:u*/alpha, /*@beta:u*/beta] = [/*@beta:u*/beta, /*@alpha:u*/alpha];

function /*@shadow:d*/shadow(/*@param:d*/alpha: number) {
  return { /*@param:u*/alpha };
}
/*@shadow:u*/shadow(/*@copy:u*/copy./*@!*/alpha);
export {};
