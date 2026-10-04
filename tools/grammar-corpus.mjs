// Deterministic grammar combinations, checked against the compiler API.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { normalizer, firstDifference } from './ast-comparison.mjs';
const require = createRequire(import.meta.url);
const ts = require(process.env.TYPE_SCRIPT_API ?? 'typescript');
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const normalize = normalizer(process.argv);
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
let passed = 0, invalid = 0;
const failures = [];
for (const { source, jsx } of cases) {
  const reference = ts.createSourceFile(jsx ? 'source.tsx' : 'source.ts', source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const result = spawnSync(binary, jsx ? ['--tsx'] : [], { input: source, encoding: 'utf8', timeout: 3000 });
  let difference;
  if (reference.parseDiagnostics.length) {
    invalid++;
    if (result.status === 0) difference = { error: 'Accepted compiler-invalid syntax', diagnostics: reference.parseDiagnostics.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')) };
  } else if (result.status !== 0) difference = { error: result.stderr.trim(), status: result.status, signal: result.signal };
  else difference = firstDifference(normalize(JSON.parse(result.stdout), true), normalize(reference, true));
  if (difference) {
    failures.push({ source, jsx, ...difference });
    if (failures.length <= 25) console.log(JSON.stringify(failures.at(-1)));
  } else passed++;
}
const report = { version: ts.version, cases: cases.length, invalid, passed, failures };
fs.mkdirSync('.cache', { recursive: true });
fs.writeFileSync('.cache/grammar-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;
