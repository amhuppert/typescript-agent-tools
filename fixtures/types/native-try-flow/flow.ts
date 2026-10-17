declare const flag: boolean;
export declare let changed: string | number;
declare function stop(): never;
declare function badEffect(value: unknown): asserts value is Missing;

export function tryCases(): void {
  const value: number = 1;
  let other: number = 0;
  try { other = 1; /*try-read*/ value; }
  catch (error) { /*catch-read*/ value; }
  /*after-catch*/ value;
  try { other = 2; }
  finally { /*finally-read*/ value; }
  /*after-finally*/ value;
  try { try { other = 3; } finally { other = 4; } }
  catch (error) { /*nested-catch*/ value; }
  /*after-nested-finally*/ value;
}

export function tryUnion(): void {
  try { changed = 1; }
  catch { /*exception-first-entry*/ changed; }
  /*normal-path-refinement*/ changed;
}

export function tryBadEffect(): void {
  const value: number = 1;
  try { badEffect(0); }
  catch { /*catch-before-call-effect*/ value; }
  /*normal-invalid-effect*/ value;
}

export function /*try-return*/ tryReturn() { try { return 1; } finally {} }
export function /*try-throw*/ tryThrow() { try { throw 1; } finally {} }
export function /*try-catch-return*/ tryCatchReturn() { try { throw 1; } catch { return 2; } finally {} }
export function /*finally-return*/ finallyReturn() { try {} finally { return 3; } }
export function /*try-never-catch*/ tryNeverCatch() { try { stop(); } catch {} }
export function /*try-never-finally*/ tryNeverFinally() { try { stop(); } finally {} }
export function /*finally-conditional-return*/ finallyConditionalReturn() {
  try { if (flag) { return 1; } }
  finally { if (!flag) { return 2; } }
}

export function /*unreachable-call-after-return*/ unreachableCallAfterReturn() { return 1; badEffect(0); }
export function /*unreachable-try-after-return*/ unreachableTryAfterReturn() { return 1; try { badEffect(0); } finally {} }
export function unreachableReadAfterReturn(): void {
  const value: number = 1;
  return;
  try { badEffect(0); } finally {}
  /*unreachable-read-after-try*/ value;
}
