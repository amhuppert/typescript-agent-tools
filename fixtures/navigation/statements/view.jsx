export function render(items) {
  return <section title={(() => { prepare(); return "ready"; })()}>
    {items.map(item => <span>{item}</span>)}
  </section>;
}
describe("JSX", () => {
  it("visits embedded statements", () => { render(["first"]); });
});
