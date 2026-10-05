// Nested shadowing, sibling scopes with equal names, and a use in the
// temporal dead zone (which still denotes the inner binding).
const /*@top*/shadowed = 1;

function /*@outer*/outer(/*@param*/shadowed: number) {
  /*@param*/shadowed;
  {
    const /*@block*/shadowed = 2;
    /*@block*/shadowed;
    {
      /*@block*/shadowed;
    }
  }
  const /*@arrow*/arrow = (/*@arrowParam*/shadowed: number) => /*@arrowParam*/shadowed;
  /*@arrow*/arrow(/*@param*/shadowed);
}

function /*@tdz*/tdz() {
  /*@tdzInner*/shadowed;
  let /*@tdzInner*/shadowed = 3;
}

function /*@first*/first() { let /*@firstLocal*/local = 1; /*@firstLocal*/local; }
function /*@second*/second() { let /*@secondLocal*/local = 2; /*@secondLocal*/local; }

/*@top*/shadowed;
/*@outer*/outer(/*@top*/shadowed);
/*@tdz*/tdz();
/*@first*/first();
/*@second*/second();
export {};
