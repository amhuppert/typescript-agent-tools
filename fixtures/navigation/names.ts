// Name forms: escapes, strings, numbers, computed and private names.
const caf\u00e9 = 1;
const __dunder = 2;
function \u{62}ravo() {}

const members = {
  plain: 1,
  'single-quoted': 2,
  "with\"quote": 3,
  "\uD800": 4,
  "\uDBFF": 5,
  "\uD83D\uDE00": 6,
  "\uFFFD": 7,
  "�": 8,
  "tab\there": 9,
  0x10: 10,
  1e3: 11,
  1_000: 12,
  .5: 13,
  [Symbol.iterator]: 14,
  ["computed"]: 15,
  [`template`]: 16,
  [42]: 17,
  [-1]: 18,
};

class Secrets {
  #secret = 1;
  static #shared() {}
  get #hidden() { return 1; }
  'string method'() {}
  42() {}
  [Symbol.toPrimitive]() {}
}

export { caf\u00e9 as "caf\u00e9 export" };
