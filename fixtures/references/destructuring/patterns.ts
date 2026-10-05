// Binding patterns introduce bindings; property keys are member names, while
// defaults and computed keys are ordinary uses.
declare const /*@source*/source: {
  /*@!*/first: number;
  /*@!*/second: { /*@!*/inner: number };
  /*@!*/pair: [number, number];
  /*@!*/optional?: number;
};
const /*@key*/key = "first";

const {
  /*@first*/first,
  /*@!*/second: { /*@!*/inner: /*@renamed*/renamed },
  /*@!*/pair: [/*@head*/head, /*@tail*/tail],
  /*@!*/optional: /*@withDefault*/withDefault = /*@first*/first,
  [/*@key*/key]: /*@computed*/computed,
} = /*@source*/source;

function /*@fn*/fn({ /*@a*/a, /*@b*/b = /*@a*/a }: { /*@!*/a: number; /*@!*/b?: number }, [/*@c*/c] = [/*@a2*/a2]) {
  return /*@a*/a + /*@b*/b + /*@c*/c;
}
const /*@a2*/a2 = 1;

/*@renamed*/renamed + /*@head*/head + /*@tail*/tail + /*@withDefault*/withDefault + /*@computed*/computed;
/*@fn*/fn({ /*@!*/a: 1 });
export {};
