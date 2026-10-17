// Exact generated compiler fixtures: boundaries are in compiler operation units,
// not source nesting depth. Keep the source generator with the oracle evidence.
export function crossProductSource(left, right) {
  const union = (prefix, length) => Array.from({ length }, (_, index) => `'${prefix}${index}'`).join(' | ');
  return `export {};\ntype Left = ${union('L', left)};\ntype Right = ${union('R', right)};\ntype /*#query*/Result = \`\${Left}\${Right}\`;\n`;
}

// The first overflowing product of two-way unions is the 63rd factor on the
// pinned 64-bit compiler. A leading Never stops size acquisition; a trailing
// Never cannot undo overflow. Do not run a large below-overflow trailing-Never
// case: the native template mapper would visit its exponential empty product.
export function crossProductOverflowSource(factors, neverPosition = 'none') {
  const spans = Array.from({ length: factors }, () => '${Choice}');
  if (neverPosition === 'first') spans.unshift('${never}');
  else if (neverPosition === 'last') spans.push('${never}');
  else if (neverPosition !== 'none') throw new Error(`Unknown Never position: ${neverPosition}`);
  return `export {};\ntype Choice = 'a' | 'b';\ntype /*#query*/Result = \`${spans.join('')}\`;\n`;
}

export function headRecursiveSource(length) {
  return `export {};\ntype Head<N extends number, T extends unknown[] = []> = T['length'] extends N ? [] : [0, ...Head<N, [...T, unknown]>];\ntype Outer<N extends number> = Head<N> extends infer U ? U : never;\ntype /*#query*/Result = Outer<${length}>;\n`;
}

// A distributive variable-N family reaches the count guard naturally. Identity
// aliases advance adjacent guard observations without setting checker state.
export function instantiationCountSource(identityAliases) {
  const union = [...Array.from({ length: 11 }, (_, index) => 900 + index), 885, 26, 2].join('|');
  const identities = Array.from({ length: identityAliases }, (_, index) => `type Id${index}<V> = V;`).join('\n');
  const argument = Array.from({ length: identityAliases }, (_, index) => index)
    .reduce((result, index) => `Id${index}<${result}>`, `Expand<${union}>`);
  return `export {};\n${identities}\ntype Build<N extends number,V,T extends unknown[] = []> = T['length'] extends N ? V : Build<N,V,[...T,V]>;\ntype Expand<V extends number> = V extends unknown ? Build<V,V> : never;\ntype /*#query*/Result = ${argument};\n`;
}

// These natural inputs preserve the pinned relation/subtype operation order.
// Annotated objects avoid the excess-property branch taken by fresh literals.
export function relationDepthSource(depth, leaf = 'same') {
  const wrap = text => '{ next: '.repeat(depth) + text + ' }'.repeat(depth);
  const targetLeaf = leaf === 'same' ? 'string' : 'number';
  return `type Source = ${wrap('{value: string}')};\ntype Target = ${wrap(`{value: ${targetLeaf}}`)};\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}

export function subtypeRemovalSource(length, removals) {
  const objects = Array.from({ length: length - removals }, (_, index) => `declare const base${index}: {tag: ${index}};`);
  const redundant = Array.from({ length: removals }, (_, index) => `declare const redundant${index}: {tag: 0; extra${index}: 0};`);
  const references = [...objects.map((_, index) => `base${index}`), ...redundant.map((_, index) => `redundant${index}`)];
  return `${[...objects, ...redundant].join('\n')}\nconst /*#query*/query = [${references.join(',')}];\n`;
}

export function relationMaybeSource(mode) {
  if (mode === 'active') return `interface Source {next: Source}\ninterface Target {next: Target}\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
  if (mode === 'active-false') return `interface Source {next: Source; value: string}\ninterface Target {next: Target; value: number}\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
  if (mode !== 'deep') throw new Error(`Unknown relation Maybe fixture: ${mode}`);
  return `type Source<T> = {next: Source<Source<T>>};\ntype Target<T> = {next: Target<Target<T>>};\ntype Query = Source<string> extends Target<number> ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}

export function relationAllowanceSource(length) {
  const type = index => `{common:string; p${index}:string}`;
  const sources = Array.from({ length }, (_, index) => type(index));
  const targets = Array.from({ length }, (_, index) => type(length - index - 1));
  return `type Source = ${sources.join('|')};\ntype Target = ${targets.join('|')};\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}
