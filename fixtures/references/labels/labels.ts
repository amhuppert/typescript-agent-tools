// Labels never enter lexical lookup, even when a variable has the same name.
let /*@flag*/flag = true;
const /*@outer*/outer = 1;

/*@!*/outer: for (;;) {
  /*@!*/inner: for (;;) {
    if (/*@flag*/flag) break /*@!*/outer;
    /*@outer*/outer;
    continue /*@!*/inner;
  }
}

/*@!*/block: {
  break /*@!*/block;
}
export {};
