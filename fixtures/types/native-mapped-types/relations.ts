export {};
type Identity<T> = { [P in keyof T]: T[P] };
type Optional<T> = { [P in keyof T]?: T[P] };
type Required<T> = { [P in keyof T]-?: T[P] };
function compare<T, K extends keyof T>(source: T, identity: Identity<T>, partial: Optional<T>, required: Required<T>) {
  const /*#sourceToMapped*/a = source satisfies Identity<T>;
  const /*#mappedToSource*/b = identity satisfies T;
  const /*#requiredToOptional*/c = required satisfies Optional<T>;
  const /*#optionalToRequired*/d = partial satisfies Required<T>;
  const /*#mappedToIndex*/e = identity satisfies { [x: string]: T[keyof T] };
  return [a, b, c, d, e];
}
function compareNames<T>(source: { [P in keyof T as P]: T[P] }) {
  const /*#sameNames*/a = source satisfies { [Q in keyof T as Q]: T[Q] };
  const /*#differentNames*/b = source satisfies { [Q in keyof T as `x${Q & string}`]: T[Q] };
  return [a, b];
}
function compareConstraints<T, K extends keyof T>(source: { [P in K]: T[P] }[K], key: keyof { [P in keyof T as `x${P & string}`]: T[P] }) {
  const /*#indexedConstraint*/a = source satisfies T[K];
  const /*#remappedKeys*/b = key satisfies string;
  return [a, b];
}
