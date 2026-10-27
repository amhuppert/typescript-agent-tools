import { initial } from "./plain.js";
export async function run({ value = initial.value } = {}) {
  await step(value);
  return { value };
}
describe("ES modules", () => {
  it("visits async bodies", async () => { await run(); });
});
