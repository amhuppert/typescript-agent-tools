// Object spreads have no entry; their expressions are transparent.
declare function build(...callbacks: Array<() => void>): object;
declare const base: object;

const spread = { ...{ run() { function nested() {} } } };
const spreadCall = { ...build(() => { function inSpread() {} }) };
export function host() {
  return {
    ...base,
    ...{ inner: { deep() {} } },
    own: 1,
  };
}
const elements = [...[function inArray() {}]];
build(...[() => { function inArguments() {} }]);
let target = {};
({ target = function fallback() {} } = {});
