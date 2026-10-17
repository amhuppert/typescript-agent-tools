export {};
function shapes<T, K extends keyof T>() {
  let /*#genericPick*/pick: Pick<T, K>;
  let /*#genericOmit*/omit: Omit<T, K>;
  let /*#genericRenamed*/renamed: ExceptFields<T, K>;
  let /*#plusModifiers*/plus: { +readonly [P in keyof T]+?: T[P] };
  let /*#minusModifiers*/minus: { -readonly [P in keyof T]-?: T[P] };
  let /*#genericName*/name: { [P in keyof T as `get${P & string}`]: T[P] };
  let /*#genericIndex*/indexed: { [P in K]: T[P] }[K];
  let /*#genericKeyof*/keys: keyof { [P in keyof T as `get${P & string}`]: T[P] };
  return [pick!, omit!, renamed!, plus!, minus!, name!, indexed!, keys!];
}
function rest<T extends { drop: number; readonly keep?: string }>(input: T) {
  const { drop, .../*#standardRest*/remaining } = input;
  return remaining;
}
