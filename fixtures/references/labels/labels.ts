// Labels never enter lexical lookup, even when a variable has the same name.
let /*@flag:d*/flag = true;
const /*@outer:d*/outer = 1;

/*@!*/outer: for (;;) {
  /*@!*/inner: for (;;) {
    if (/*@flag:u*/flag) break /*@!*/outer;
    /*@outer:u*/outer;
    continue /*@!*/inner;
  }
}

/*@!*/block: {
  break /*@!*/block;
}
export {};

// Meta-property names are not references either.
const /*@target:d*/target = 1;
function /*@meta:d*/meta() { return new./*@!*/target ?? import./*@!*/meta; }
/*@target:u*/target; /*@meta:u*/meta;
