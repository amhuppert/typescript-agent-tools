// Parameters versus body declarations. Parameter initializers see earlier
// parameters and the enclosing scope, not the body's own declarations.
const /*@outerX*/x = 1;

function /*@defaults*/defaults(/*@a*/a = /*@outerX*/x, /*@b*/b = /*@a*/a) {
  var /*@bodyX*/x = 2;
  return /*@a*/a + /*@b*/b + /*@bodyX*/x;
}

function /*@redeclare*/redeclare(/*@value*/value: number) {
  var /*@value*/value: number;
  return /*@value*/value;
}

function /*@nested*/nested(/*@item*/item: number) {
  {
    let /*@blockItem*/item = 2;
    /*@blockItem*/item;
  }
  return /*@item*/item;
}

const /*@arrow*/arrow = (/*@first*/first: number, .../*@rest*/rest: any[]) => /*@first*/first ?? /*@rest*/rest;
const /*@holder*/holder = { /*@!*/method(/*@methodParam*/input: number) { return /*@methodParam*/input; } };

type /*@Callback*/Callback = (/*@typeParam*/argument: number) => void;
let /*@callback*/callback: /*@Callback*/Callback = (/*@used*/argument) => { /*@used*/argument; };

/*@defaults*/defaults();
/*@redeclare*/redeclare(/*@outerX*/x);
/*@nested*/nested(1);
/*@arrow*/arrow(1);
/*@holder*/holder./*@!*/method(2);
/*@callback*/callback(3);
export {};
