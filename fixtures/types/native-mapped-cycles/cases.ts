export {};
type Mirror<T> = { [P in keyof T as P]: string };
declare function choose<T>(key: keyof Mirror<T>): 1;
declare function choose<T>(key: unknown): 2;
function recursive<T extends Mirror<T>, U extends string>(key: U, text: string, known: keyof T) {
  const /*#literal*/literal = choose<T>('x');
  const /*#text*/broad = choose<T>(text);
  const /*#constrained*/constrained = choose<T>(key);
  const /*#known*/original = choose<T>(known);
  return [literal, broad, constrained, original];
}
type Renamed<T> = { [P in keyof T as `x${P & string}`]: string };
declare function renamed<T>(key: keyof Renamed<T>): 1;
declare function renamed<T>(key: unknown): 2;
function recursiveRenamed<T extends Renamed<T>>(key: string, known: keyof T) {
  const /*#renamedLiteral*/literal = renamed<T>('xx');
  const /*#renamedText*/broad = renamed<T>(key);
  const /*#renamedKnown*/original = renamed<T>(known);
  return [literal, broad, original];
}
