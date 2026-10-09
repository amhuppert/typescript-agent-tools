// JSDoc @typedef and @callback tags declare nothing in TypeScript files.
/** @typedef {string} Name */
export const named = 'x';

/**
 * @callback Handler
 * @param {string} value
 * @returns {void}
 */
export function handle(value: string): void {
  /** @typedef {{ inner: number }} Local */
  const local = 1;
}

/** An ordinary comment that mentions @typedef is still documentation. */
export class Documented {}
