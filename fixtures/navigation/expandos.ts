// Expando assignments and same-named properties, which the compiler merges.
function target() {}
target.extra = 1;
target.callback = () => {
  const inCallback = 1;
};

function createHost() {
  function startEligible() {}
  const run = 1;
  return {
    startEligible,
    run: () => {
      function nested() {}
    },
    other() {},
  };
}

class Counter {
  get size() { return 1; }
  size2 = 1;
}
function size() {}

const config = { retries: 1 };
call({ config: 2, standalone: 3 });
declare function call(value: object): void;
target.method = function () {
  function keepMe() {}
};
function outer() {
  function inner() {}
  inner.attached = function () {
    function keepInner() {}
  };
}
