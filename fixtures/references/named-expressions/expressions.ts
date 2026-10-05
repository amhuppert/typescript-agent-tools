// A named function or class expression's name is visible only inside it.
const /*@factorial*/factorial = function /*@fact*/fact(/*@n*/n: number): number {
  return /*@n*/n ? /*@fact*/fact(/*@n*/n - 1) : 1;
};
/*@factorial*/factorial(3);
/*@?*/fact;

const /*@Klass*/Klass = class /*@Named*/Named {
  /*@!*/method() { return /*@Named*/Named; }
};
new /*@Klass*/Klass();
/*@?*/Named;

const /*@same*/same = function /*@sameInner*/same() { return /*@sameInner*/same; };
/*@same*/same();
export {};
