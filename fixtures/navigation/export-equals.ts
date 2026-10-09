// `export =` with an object, and an assigned function expression.
declare let handler: () => void;
handler = function () {
  const assignedLocal = 1;
};

export = {
  run() {},
  nested: { deep: 1 },
};
