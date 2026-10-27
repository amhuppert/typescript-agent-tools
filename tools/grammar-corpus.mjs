// Deterministic grammar combinations, checked against the compiler API.
import fs from 'node:fs';
import path from 'node:path';
import { COMPARISON_OPTIONS, normalizer, describeComparison } from './ast-comparison.mjs';
import { loadCompilerApi, compareSource } from './reference-oracle.mjs';
import { runnerArguments, requireComparisons } from './corpus-selection.mjs';
const { flags } = runnerArguments(process.argv.slice(2), { options: COMPARISON_OPTIONS });
const normalize = normalizer(flags);
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const cases = [];
const add = (source, jsx = false) => cases.push({ source, jsx });
const atoms = ['x', '1', '"a"', 'true', 'f<T>()', '(x)', '[x, ...xs]', '{x: 1}', 'x?.y', '`a${x}`'];
const operators = ['+', '-', '*', '/', '%', '**', '<', '>', '<=', '>=', '==', '!=', '===', '!==', '&', '|', '^', '<<', '>>', '>>>', '&&', '||', '??', 'in', 'instanceof', '=', '+=', '&&=', '??=', ','];
for (let i = 0; i < atoms.length; i++) for (let j = 0; j < operators.length; j++) {
  const a = atoms[i], b = atoms[(i + j + 1) % atoms.length];
  add(`const result = (${a}) ${operators[j]} (${b});`);
  add(`const result = (${a}) ${operators[j]} (${b}) ${operators[(j + 7) % operators.length]} x;`);
}
const types = ['string', 'T', 'T[K]', 'T[]', 'readonly T[]', 'keyof T', 'Promise<T>', '[T, U?]', '{x: T}', '() => T', '(x: infer U) => T', 'infer U', 'infer U extends X', 'typeof value', '`${T}`', 'unknown', 'T | U', 'T & U'];
for (const type of types) {
  add(`type A<T, U, K> = ${type};`);
  add(`type A<T, U, K> = T extends ${type} ? T : never;`);
  add(`type A<T, U, K> = { [P in keyof T as P]: ${type} };`);
  add(`type A<T, U, K> = [x: ${type}, ...rest: U[]];`);
  add(`type A<T, U, K> = (${type})[];`);
}
for (const prefix of ['', 'async ', 'function* f() { ', 'async function* f() { ']) {
  const functionPrefix = prefix.includes('function');
  for (const expression of ['await x', 'await(x)', 'await\n(x)', 'yield x', 'yield* xs', 'yield', 'await /x/', 'await import("pkg")', 'await (x => x)(y)']) {
    add(`${prefix}${functionPrefix ? '' : 'function f() { '}${expression}; }`);
  }
}
for (const expression of ['await(x)', 'await\n(x)', 'await\nx', 'await x', 'await /x/', 'await (x => x)(y)', 'await +x', 'await (async () => 1)()', 'for await (const x of xs) {}']) {
  add(`export {}; ${expression}${expression.endsWith('}') ? '' : ';'}`);
  add(`${expression}${expression.endsWith('}') ? '' : ';'} export {};`);
}
for (const source of [
  'const x = { await: 1 }; export {};', 'const x = obj.await; export {};',
  'const x = await(x); const y = 1; export {};', 'if (x) { await(foo); } export {};',
  'export default await(foo);', 'export = import("pkg");',
  'class C { static await = 1; } export {};', 'class C { await = 1; } export {};',
  'class C extends await(foo) {} export {};', 'export class C extends await(foo) {}',
  'const fn = () => await(foo); export {};', 'const fn = async () => await(foo); export {};',
  'namespace A { const x = await(foo); } export {};',
  'import type from "pkg";', 'import type from from "pkg";', 'import type type from "pkg";',
  'import {type, type as t, type as as, type Foo as Bar} from "pkg";',
  'export {type, type as t, type as as, type Foo as Bar};',
  'let\nx = 1;', 'using\nx = acquire();', 'async\nfunction f() {}',
  'const fn = (async) => async;', 'const fn = async => async;',
  'const x = { async() {}, get() {}, set() {}, async get() {}, get x() { return 1; } };',
  'class C { static() {} readonly() {} declare() {} override() {} async() {} accessor() {} }',
  'class C { static get x() { return 1; } accessor x: number = 1; async *f() { yield 1; } }',
  'type A = number | (() => void); type B = (() => void) & {x: 1};',
  'type A = number | () => void;', 'type A = () => void | number;',
  'const x = f<T> + y;', 'const x = f<T> - y;', 'const x = new C<T>.member();',
  'const x = f<T> * y;', 'const x = f<T> / y;', 'const x = f<T>[y];',
  'const x = f<T> % y;', 'const x = f<T> instanceof y;', 'f<T> = y;',
  'const x = f<<T>(x: T) => T>();', 'const x = f?.<<T>() => T>();',
  'type X = Wrapper<<T>() => T>;', 'type X = typeof f<<T>() => T>;',
  'type X = import("pkg").F<<T>() => T>;', 'const x = <UI<<T>() => T> />;',
  'type X = typeof f< <T>() => T>;', 'const x = <UI< <T>() => T> />;',
  'const x = tag<T>`hello`;', 'const x = fn\n<T>(x);',
  'const x = 0.; const y = .25e+2;', 'const x = 1e400;',
  'const x = <p> </p>;', 'const x = <p>\n\t </p>;',
  'const x = <p>\n\u00a0 </p>;', 'const x = <p>\v\n </p>;',
  'const x = <UI . Button />;', 'const x = <x : tag />;',
  'const x = <p a="\\" b="line\nbreak">//text</p>;',
  'const x = <p>{await(foo)}</p>; export {};',
  'const x = <__tag><UI.__Thing /></__tag>;',
  'const x = <T extends U>(x: T) => x;', 'const x = <T extends U = U>(x: T) => x;',
  'const for = 1;', 'function return() {}', 'type for = T;', 'class for {}',
  'const x = {default};', 'const {default} = x;', 'const {"x"} = x;',
  'const {default: x} = y;', 'const x = {default: 1};', 'const x = obj.for;',
  'const \\u0066or = 1;', 'const \\u0061sync = 1;', 'import for from "pkg";',
]) add(source, /<(?:p|UI|x\s*:|T extends)/.test(source));
// TypeScript 7 parses a namespace declared with `module`, whose tree is
// compared while --check must reject it (TS1540); the keyword decides whether
// a string name, a dotted name, or a missing block is allowed.
for (const source of [
  'module X {}', 'declare module X { const a: number; }', 'export module X { export const a = 1; }',
  'module A.B.C { export const a = 1; }', 'declare module A.B {}', 'namespace A.B { module C {} }',
  'function f() { module X {} }', 'module "x" {}', 'declare module "x";', 'export {}; declare global {}',
  'module\nX\n{}', 'module\nX {}', 'export module\nX {}', 'export namespace\nX {}',
  'module X;', 'namespace X;', 'module A.B;', 'declare namespace "x" {}', 'declare module "x".y {}',
  // Names after a dot are identifier names (parseIdentifierName); the first is a binding identifier.
  'export declare module A.default { const n: number; }', 'export declare module A.\\u0064efault { const n: number; }',
  'namespace A.class {}', 'namespace A.cl\\u0061ss.if {}', 'namespace A.\\u{64}efault {}', 'module A.await.yield {}',
  'namespace A.#x {}', 'namespace A.#\\u0078 {}', 'namespace A.#\\u{78} {}', 'namespace A.#\\u{78}.\\u{79} {}',
  'module A.#x {}', 'module A.#\\u0078 {}', 'module A.#\\u{78} {}', 'namespace default {}', 'namespace \\u0064efault {}',
]) add(source);
let passed = 0, invalid = 0;
const failures = [];
for (const { source, jsx } of cases) {
  const facts = compareSource(ts, binary, { name: jsx ? 'source.tsx' : 'source.ts', source, normalize, options: { timeout: 3000 } });
  // Authored combinations must use syntax TypeScript 7 still has; an invalid
  // combination passes when the parser rejects it.
  if (!facts.eligible) {
    failures.push({ source, jsx, error: `Ineligible authored fixture: ${facts.removed}` });
    continue;
  }
  if (!facts.valid) invalid++;
  if (facts.difference) {
    failures.push({ source, jsx, ...facts.difference });
    if (failures.length <= 25) console.log(JSON.stringify(failures.at(-1)));
  } else passed++;
}
requireComparisons(cases.length);
const report = { version: ts.version, compared: describeComparison(normalize.compared), cases: cases.length, invalid, passed, failures };
fs.mkdirSync('.cache', { recursive: true });
fs.writeFileSync('.cache/grammar-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;
