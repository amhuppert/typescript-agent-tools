export interface Host {
  /*#fooSeed*/foo: number;
  /*#decodedSeed*/decoded: number;
  /*#numericSeed*/16: number;
  /*#emptySeed*/"": number;
}
declare const host: Host;
const /*#fooLocal*/foo = 1;
const named = { /*#fooAssignment*/foo: 1 };
const shorthand = { /*#fooShorthand*/foo };
class ParameterHost {
  constructor(/*#fooModifier*/public /*#fooParameter*/foo: number) {}
  read() { return this./*#fooParameterUse*/foo; }
}
const computed = {
  [/*#fooComputedLiteral*/"foo"]: 1,
  [/*#fooComputedVariable*/foo]: 2
};
host[/*#fooElement*/"foo"];
host[/*#fooComputedArgument*/foo];
host./*#fooEscaped*/\u0066oo;
const decoded = { /*#decodedAssignment*/"deco\u0064ed": 1 };
host[/*#decodedElement*/"deco\u0064ed"];
const numeric = { /*#numericAssignment*/0x10: 1 };
host[/*#numericElement*/0x10];
const empty = { /*#emptyAssignment*/"": 1 };
host[/*#emptyElement*/""];
declare function Panel(props: { /*#fooProps*/foo: number }): any;
const tags = { /*#fooTagDeclaration*/foo: Panel };
<Panel foo={1} />;
<tags.foo foo={1} />;
class PrivateHost {
  /*#privateSeed*/#foo = 1;
  read() { return this./*#privateUse*/#foo; }
}
interface NamespacedProps { /*#namespacedSeed*/"ns:foo": number; }
declare function NamespacedPanel(props: NamespacedProps): any;
<NamespacedPanel ns:foo={1} />;
