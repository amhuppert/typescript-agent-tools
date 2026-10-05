#!/usr/bin/env node
// Deterministic multi-file projects that combine declaration kinds and valid
// merges, re-export and import forms, shadowing, and value, type, typeof, and
// qualified uses. Compare them with the semantic oracle:
//
//   node tools/reference-combinations.mjs [--count=N] [--seed=S] [--out=DIR]
//   node tools/semantic-conformance.mjs compare DIR/*
//
// Names come from a small pool so that imports, locals, and parameters
// collide. Declarations that cannot share a name are never generated, so
// every project binds; unresolved uses are deliberate.
import fs from 'node:fs';
import path from 'node:path';

const NAMES = ['alpha', 'beta', 'Gamma', 'Delta'];

// Each kind's meanings and the kinds it may share a name with in one module.
const KINDS = {
  const: { meanings: ['value'], code: n => `export const ${n} = 1;` },
  function: { meanings: ['value'], code: n => `export function ${n}(input: number) { return input; }` },
  class: { meanings: ['value', 'type'], code: n => `export class ${n} { size = 1; static make() { return new ${n}(); } }` },
  interface: { meanings: ['type'], code: n => `export interface ${n} { size: number }` },
  type: { meanings: ['type'], code: n => `export type ${n} = { kind: string };` },
  enum: { meanings: ['value', 'type', 'namespace'], code: n => `export enum ${n} { First, Second }` },
  namespace: { meanings: ['value', 'namespace'], code: n => `export namespace ${n} { export const inner = 1; export type Inner = number; }` },
};
// Two namespace blocks would redeclare their identical members.
const COMPATIBLE = new Set([
  'interface+interface', 'class+interface', 'function+namespace', 'class+namespace', 'enum+namespace',
  'const+interface', 'const+type', 'function+interface', 'function+type',
]);
const compatible = (a, b) => COMPATIBLE.has(`${a}+${b}`) || COMPATIBLE.has(`${b}+${a}`);

function random(seed) {
  let state = seed >>> 0 || 1;
  const next = () => {
    state ^= state << 13; state >>>= 0;
    state ^= state >>> 17;
    state ^= state << 5; state >>>= 0;
    return state / 0x100000000;
  };
  return {
    int: n => Math.floor(next() * n),
    pick: items => items[Math.floor(next() * items.length)],
    chance: p => next() < p,
  };
}

function declarations(rng) {
  const declared = new Map();
  const lines = [];
  for (let attempt = 0; attempt < 8; attempt++) {
    const name = rng.pick(NAMES);
    const kind = rng.pick(Object.keys(KINDS));
    const existing = declared.get(name) ?? [];
    if (!existing.every(other => compatible(other, kind))) continue;
    if (kind === 'class' && existing.includes('class')) continue;
    // A namespace must follow the class or function it merges with.
    if ((kind === 'class' || kind === 'function') && existing.includes('namespace')) continue;
    declared.set(name, [...existing, kind]);
    lines.push(KINDS[kind].code(name));
  }
  return { declared, lines };
}

function meanings(kinds) {
  return new Set(kinds.flatMap(kind => KINDS[kind]?.meanings ?? ['value', 'namespace']));
}

// A barrel re-exporting some of `a.ts` under several forms.
function barrel(rng, declared) {
  const lines = [];
  const exported = new Map();
  if (rng.chance(0.5)) {
    lines.push('export * from "./a";');
    for (const [name, kinds] of declared) exported.set(name, { kinds, typeOnly: false });
  } else {
    for (const [name, kinds] of declared) {
      if (!rng.chance(0.7)) continue;
      const alias = rng.chance(0.4) ? `${name}Alias` : name;
      const typeOnly = rng.chance(0.25);
      lines.push(`export ${typeOnly ? 'type ' : ''}{ ${alias === name ? name : `${name} as ${alias}`} } from "./a";`);
      exported.set(alias, { kinds, typeOnly });
    }
  }
  if (rng.chance(0.5)) {
    lines.push('export * as all from "./a";');
    exported.set('all', { kinds: ['module'], typeOnly: false });
  }
  return { lines, exported };
}

function uses(name, kinds, rng, local) {
  const lines = [];
  const has = meanings(kinds);
  if (kinds.includes('module')) {
    const [inner] = local;
    if (inner) lines.push(`${name}.${inner};`);
    return lines;
  }
  if (has.has('value')) lines.push(rng.pick([`${name};`, `console.log(${name});`, `const copy${rng.int(1_000_000)} = ${name};`]));
  if (has.has('type')) lines.push(`let typed${rng.int(1_000_000)}: ${name} | undefined;`);
  if (has.has('value')) lines.push(`let queried${rng.int(1_000_000)}: typeof ${name} | undefined;`);
  if (has.has('namespace') && kinds.includes('namespace')) {
    lines.push(`${name}.inner;`, `let qualified${rng.int(1_000_000)}: ${name}.Inner | undefined;`);
  }
  return lines;
}

function consumer(rng, source, exported, declaredNames) {
  const imports = [];
  const body = [];
  const visible = new Map();
  for (const [name, entry] of exported) {
    if (!rng.chance(0.75)) continue;
    const local = rng.chance(0.3) ? `${name}Local` : name;
    if (visible.has(local)) continue;
    if (entry.kinds.includes('module')) {
      imports.push(`import { ${name === local ? name : `${name} as ${local}`} } from "${source}";`);
    } else if (rng.chance(0.2)) {
      imports.push(`import type { ${name === local ? name : `${name} as ${local}`} } from "${source}";`);
      entry.typeOnly = true;
    } else {
      imports.push(`import { ${name === local ? name : `${name} as ${local}`} } from "${source}";`);
    }
    visible.set(local, entry);
  }
  if (rng.chance(0.5)) {
    imports.push(`import * as space from "${source}";`);
    for (const [name] of exported) if (rng.chance(0.5)) body.push(`space.${name};`);
  }
  for (const [local, entry] of visible) body.push(...uses(local, entry.kinds, rng, [...declaredNames]));
  // Shadowing: parameters, block locals, and type parameters that reuse names.
  for (const [local, entry] of visible) {
    if (!rng.chance(0.5)) continue;
    switch (rng.int(3)) {
      case 0:
        body.push(`function shadow${body.length}(${local}: number) { return ${local} + 1; }`);
        break;
      case 1:
        body.push(`{ const ${local} = "inner"; ${local}.length; }`);
        break;
      default:
        body.push(`function generic${body.length}<${local}>(value: ${local}): ${local} { return value; }`);
    }
    body.push(...uses(local, entry.kinds, rng, [...declaredNames]).slice(0, 1));
  }
  return [...imports, ...body, 'export {};'];
}

function project(seed) {
  const rng = random(seed);
  const { declared, lines } = declarations(rng);
  const files = { 'a.ts': lines.join('\n') + '\n' };
  const { lines: barrelLines, exported } = barrel(rng, declared);
  files['b.ts'] = barrelLines.join('\n') + '\nexport {};\n';
  const fromBarrel = rng.chance(0.6);
  const direct = new Map([...declared].map(([name, kinds]) => [name, { kinds, typeOnly: false }]));
  files['c.ts'] = consumer(rng, fromBarrel ? './b' : './a', fromBarrel ? exported : direct, declared.keys()).join('\n') + '\n';
  files['tsconfig.json'] = '{ "compilerOptions": { "noLib": true, "strict": true } }\n';
  return files;
}

function main() {
  const options = Object.fromEntries(process.argv.slice(2).map(arg => {
    const match = /^--(count|seed|out)=(.+)$/.exec(arg);
    if (!match) {
      console.error('usage: reference-combinations.mjs [--count=N] [--seed=S] [--out=DIR]');
      process.exit(2);
    }
    return [match[1], match[2]];
  }));
  const count = Number(options.count ?? 200);
  const seed = Number(options.seed ?? 1);
  if (!Number.isInteger(count) || count < 1 || !Number.isInteger(seed)) {
    console.error('--count must be a positive integer and --seed an integer');
    process.exit(2);
  }
  const out = path.resolve(options.out ?? '.cache/reference-combinations');
  fs.rmSync(out, { recursive: true, force: true });
  for (let index = 0; index < count; index++) {
    const directory = path.join(out, String(index).padStart(4, '0'));
    fs.mkdirSync(directory, { recursive: true });
    for (const [name, text] of Object.entries(project(seed * 100_003 + index))) fs.writeFileSync(path.join(directory, name), text);
  }
  console.log(JSON.stringify({ projects: count, seed, out }));
}

main();
