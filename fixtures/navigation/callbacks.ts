// Callback labels, anonymous functions, and named function expressions.
declare function describe(name: string, callback: () => void): void;
declare function it(name: string, callback: () => void): void;
declare const suite: { each(table: string): (name: string, callback: () => void) => void };
declare const tname: string;

describe('runs', () => {
  it("starts", () => {
    const started = true;
  });
  it(`stops ${tname}`, async () => {});
  describe(`plain`, function () {
    it('multi\
line', () => {});
  });
});

suite.each('table')('row', () => {});
[1, 2].map((item) => item * 2);
Promise.resolve().then(function () {});
foo(function namedExpression() {
  const insideNamed = 1;
});
new Factory(() => {});
(function () {
  function hidden() {}
})();
const parenthesized = (() => {
  function insideParenthesized() {}
});
const conditional = Math.random() ? () => {} : function () {};
export default (() => {});

declare function foo(callback: () => void): void;
declare class Factory { constructor(callback: () => void) }
