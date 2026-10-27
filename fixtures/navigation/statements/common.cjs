function create(value) {
  return { value, update(next) { return create(next); } };
}
module.exports = create;
describe("CommonJS", () => {
  const instance = create(1);
  it("visits object methods", () => { instance.update(2); });
});
