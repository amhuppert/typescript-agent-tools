// Function-scoped `var` versus block-scoped `let`/`const`/`using`, loops,
// switch clauses, catch bindings, and class static blocks.
declare const /*@source:d*/source: { /*@!*/one: number };
declare const /*@list:d*/list: any;

function /*@scopes:d*/scopes() {
  /*@hoisted:u*/hoisted;
  var /*@hoisted:d*/hoisted = 1;
  let /*@outer:d*/value = 1;
  {
    let /*@inner:d*/value = 2;
    const /*@constant:d*/constant = /*@inner:u*/value;
    /*@constant:u*/constant;
    var /*@hoisted:d*/hoisted = 3;
  }
  /*@outer:u*/value;
  for (let /*@index:d*/index = 0; /*@index:u*/index < 1; /*@index:u*/index++) { /*@index:u*/index; }
  for (const /*@key:d*/key in /*@source:u*/source) { /*@key:u*/key; }
  for (const /*@item:d*/item of /*@list:u*/list) { /*@item:u*/item; }
  for (var /*@counter:d*/counter = 0; /*@counter:u*/counter < 1; /*@counter:u*/counter++) {}
  /*@counter:u*/counter;
  while (/*@hoisted:u*/hoisted) { let /*@loopLocal:d*/loopLocal = 1; /*@loopLocal:u*/loopLocal; break; }
  switch (/*@hoisted:u*/hoisted) {
    case 1:
      let /*@clause:d*/clause = 1;
      /*@clause:u*/clause;
      break;
    default:
      /*@clause:u*/clause;
  }
  try {
    /*@hoisted:u*/hoisted;
  } catch (/*@caught:d*/error) {
    /*@caught:u*/error;
    var /*@fromCatch:d*/fromCatch = 1;
  }
  /*@fromCatch:u*/fromCatch;
  {
    using /*@resource:d*/resource = null;
    /*@resource:u*/resource;
  }
}

async function /*@later:d*/later() {
  await using /*@awaited:d*/awaited = null;
  /*@awaited:u*/awaited;
}

class /*@Statics:d*/Statics {
  static {
    var /*@inStatic:d*/inStatic = 1;
    let /*@blockLocal:d*/blockLocal = /*@inStatic:u*/inStatic;
    /*@blockLocal:u*/blockLocal;
  }
}

/*@?*/inStatic;
/*@scopes:u*/scopes();
/*@later:u*/later();
new /*@Statics:u*/Statics();
export {};
