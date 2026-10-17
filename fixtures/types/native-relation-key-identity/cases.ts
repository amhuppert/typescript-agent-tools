export {};
declare global { interface Object {} interface String {} interface Number {} }
interface Source<T> { value: T; }
interface Target<T> { value: T; }
interface OtherTarget<T> { value: T; required: number; }
interface Box<T> { item: T; }
function parameterProbe<T, U>(
  source: Source<T>,
  warm: { (value: Target<T>): number; (value: unknown): number; },
  choose: { (value: Target<U>): "wrong"; (value: Source<T>): number; },
) {
  const parameterChosen = (warm(source), choose(source));
}
function targetProbe<T>(
  source: Source<T>,
  warm: { (value: Target<T>): number; (value: unknown): number; },
  choose: { (value: OtherTarget<T>): "wrong"; (value: Source<T>): number; },
) {
  const targetChosen = (warm(source), choose(source));
}
function nestedProbe<T>(
  source: Source<Box<T>>,
  warm: { (value: Target<Box<T>>): number; (value: unknown): number; },
  choose: { (value: Target<T>): "wrong"; (value: Source<Box<T>>): number; },
) {
  const nestedChosen = (warm(source), choose(source));
}
function constrainedProbe<T extends string, U extends number>(
  source: Source<T>,
  warm: { (value: Target<T>): number; (value: unknown): number; },
  choose: { (value: Target<U>): "wrong"; (value: Source<T>): number; },
) {
  const constrainedChosen = (warm(source), choose(source));
}
