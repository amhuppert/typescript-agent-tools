// Binding patterns introduce bindings; property keys are member names, while
// defaults and computed keys are ordinary uses.
declare const /*@source:d*/source: {
  /*@!*/first: number;
  /*@!*/second: { /*@!*/inner: number };
  /*@!*/pair: [number, number];
  /*@!*/optional?: number;
};
const /*@key:d*/key = "first";

const {
  /*@first:d*/first,
  /*@!*/second: { /*@!*/inner: /*@renamed:d*/renamed },
  /*@!*/pair: [/*@head:d*/head, /*@tail:d*/tail],
  /*@!*/optional: /*@withDefault:d*/withDefault = /*@first:u*/first,
  [/*@key:u*/key]: /*@computed:d*/computed,
} = /*@source:u*/source;

function /*@fn:d*/fn({ /*@a:d*/a, /*@b:d*/b = /*@a:u*/a }: { /*@!*/a: number; /*@!*/b?: number }, [/*@c:d*/c] = [/*@a2:u*/a2]) {
  return /*@a:u*/a + /*@b:u*/b + /*@c:u*/c;
}
const /*@a2:d*/a2 = 1;

/*@renamed:u*/renamed + /*@head:u*/head + /*@tail:u*/tail + /*@withDefault:u*/withDefault + /*@computed:u*/computed;
/*@fn:u*/fn({ /*@!*/a: 1 });
export {};

// Object rest elements, array holes, and a default that reads an earlier
// element; shorthand properties read the bindings.
const { /*@!*/first: /*@firstAgain:d*/firstAgain, .../*@others:d*/others } = /*@source:u*/source;
const [/*@lead:d*/lead, , /*@third:d*/third = /*@lead:u*/lead] = [1, 2, 3];
const /*@gathered:d*/gathered = { /*@others:u*/others, /*@third:u*/third, /*@firstAgain:u*/firstAgain };
/*@gathered:u*/gathered;
