// Variable bindings: multiple, destructured, loops, and catch clauses.
declare const source: { a: number; b: number; nested: { c: number }; rest: number[] };
let first = 1, second = () => { function inSecond() {} };
const { a, b: renamed, nested: { c }, ...others } = source;
const [head, , tail = () => {}] = [1, 2, () => {}];
var legacy;

for (const item of [1, 2]) {
  const perItem = item;
}
for (let index = 0, limit = 2; index < limit; index++) {}
for (const key in source) {}
try {
  throw new Error();
} catch (failure) {
  const handled = failure;
}

function locals(flag: boolean) {
  if (flag) {
    const shadowed = 1;
  } else {
    const shadowed = 2;
  }
}
try {} catch ({ message, cause: reason }) {
  const described = message;
}
