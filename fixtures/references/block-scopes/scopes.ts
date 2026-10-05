// Function-scoped `var` versus block-scoped `let`/`const`/`using`, loops,
// switch clauses, catch bindings, and class static blocks.
declare const /*@source*/source: { /*@!*/one: number };
declare const /*@list*/list: any;

function /*@scopes*/scopes() {
  /*@hoisted*/hoisted;
  var /*@hoisted*/hoisted = 1;
  let /*@outer*/value = 1;
  {
    let /*@inner*/value = 2;
    const /*@constant*/constant = /*@inner*/value;
    /*@constant*/constant;
    var /*@hoisted*/hoisted = 3;
  }
  /*@outer*/value;
  for (let /*@index*/index = 0; /*@index*/index < 1; /*@index*/index++) { /*@index*/index; }
  for (const /*@key*/key in /*@source*/source) { /*@key*/key; }
  for (const /*@item*/item of /*@list*/list) { /*@item*/item; }
  for (var /*@counter*/counter = 0; /*@counter*/counter < 1; /*@counter*/counter++) {}
  /*@counter*/counter;
  while (/*@hoisted*/hoisted) { let /*@loopLocal*/loopLocal = 1; /*@loopLocal*/loopLocal; break; }
  switch (/*@hoisted*/hoisted) {
    case 1:
      let /*@clause*/clause = 1;
      /*@clause*/clause;
      break;
    default:
      /*@clause*/clause;
  }
  try {
    /*@hoisted*/hoisted;
  } catch (/*@caught*/error) {
    /*@caught*/error;
    var /*@fromCatch*/fromCatch = 1;
  }
  /*@fromCatch*/fromCatch;
  {
    using /*@resource*/resource = null;
    /*@resource*/resource;
  }
}

async function /*@later*/later() {
  await using /*@awaited*/awaited = null;
  /*@awaited*/awaited;
}

class /*@Statics*/Statics {
  static {
    var /*@inStatic*/inStatic = 1;
    let /*@blockLocal*/blockLocal = /*@inStatic*/inStatic;
    /*@blockLocal*/blockLocal;
  }
}

/*@?*/inStatic;
/*@scopes*/scopes();
/*@later*/later();
new /*@Statics*/Statics();
export {};
