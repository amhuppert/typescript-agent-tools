/** @type {{ value: number }} */
export const initial = { value: 1 };
describe("plain JavaScript", () => {
  it("visits accessors in order", () => {
    const view = {
      get value() { return initial.value; },
      set value(next) { initial.value = next; },
    };
    view.value += 1;
  });
});
