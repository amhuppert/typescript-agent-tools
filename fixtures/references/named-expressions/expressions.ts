// A named function or class expression's name is visible only inside it.
const /*@factorial:d*/factorial = function /*@fact:d*/fact(/*@n:d*/n: number): number {
  return /*@n:u*/n ? /*@fact:u*/fact(/*@n:u*/n - 1) : 1;
};
/*@factorial:u*/factorial(3);
/*@?*/fact;

const /*@Klass:d*/Klass = class /*@Named:d*/Named {
  /*@!*/method() { return /*@Named:u*/Named; }
};
new /*@Klass:u*/Klass();
/*@?*/Named;

const /*@same:d*/same = function /*@sameInner:d*/same() { return /*@sameInner:u*/same; };
/*@same:u*/same();
export {};
