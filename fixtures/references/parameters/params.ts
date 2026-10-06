// Parameters versus body declarations. Parameter initializers see earlier
// parameters and the enclosing scope, not the body's own declarations.
const /*@outerX:d*/x = 1;

function /*@defaults:d*/defaults(/*@a:d*/a = /*@outerX:u*/x, /*@b:d*/b = /*@a:u*/a) {
  var /*@bodyX:d*/x = 2;
  return /*@a:u*/a + /*@b:u*/b + /*@bodyX:u*/x;
}

function /*@redeclare:d*/redeclare(/*@value:d*/value: number) {
  var /*@value:d*/value: number;
  return /*@value:u*/value;
}

function /*@nested:d*/nested(/*@item:d*/item: number) {
  {
    let /*@blockItem:d*/item = 2;
    /*@blockItem:u*/item;
  }
  return /*@item:u*/item;
}

const /*@arrow:d*/arrow = (/*@first:d*/first: number, .../*@rest:d*/rest: any[]) => /*@first:u*/first ?? /*@rest:u*/rest;
const /*@holder:d*/holder = { /*@!*/method(/*@methodParam:d*/input: number) { return /*@methodParam:u*/input; } };

type /*@Callback:d*/Callback = (/*@typeParam:d*/argument: number) => void;
let /*@callback:d*/callback: /*@Callback:u*/Callback = (/*@used:d*/argument) => { /*@used:u*/argument; };

/*@defaults:u*/defaults();
/*@redeclare:u*/redeclare(/*@outerX:u*/x);
/*@nested:u*/nested(1);
/*@arrow:u*/arrow(1);
/*@holder:u*/holder./*@!*/method(2);
/*@callback:u*/callback(3);
export {};

// `arguments` is built in, not a reference target, in initializers and bodies.
function /*@implicit:d*/implicit(/*@implicitFirst:d*/first = /*@!*/arguments) {
  return [/*@implicitFirst:u*/first, /*@!*/arguments, () => /*@!*/arguments];
}
/*@implicit:u*/implicit;
