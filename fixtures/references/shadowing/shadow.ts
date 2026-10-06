// Nested shadowing, sibling scopes with equal names, and a use in the
// temporal dead zone (which still denotes the inner binding).
const /*@top:d*/shadowed = 1;

function /*@outer:d*/outer(/*@param:d*/shadowed: number) {
  /*@param:u*/shadowed;
  {
    const /*@block:d*/shadowed = 2;
    /*@block:u*/shadowed;
    {
      /*@block:u*/shadowed;
    }
  }
  const /*@arrow:d*/arrow = (/*@arrowParam:d*/shadowed: number) => /*@arrowParam:u*/shadowed;
  /*@arrow:u*/arrow(/*@param:u*/shadowed);
}

function /*@tdz:d*/tdz() {
  /*@tdzInner:u*/shadowed;
  let /*@tdzInner:d*/shadowed = 3;
}

function /*@first:d*/first() { let /*@firstLocal:d*/local = 1; /*@firstLocal:u*/local; }
function /*@second:d*/second() { let /*@secondLocal:d*/local = 2; /*@secondLocal:u*/local; }

/*@top:u*/shadowed;
/*@outer:u*/outer(/*@top:u*/shadowed);
/*@tdz:u*/tdz();
/*@first:u*/first();
/*@second:u*/second();
export {};
