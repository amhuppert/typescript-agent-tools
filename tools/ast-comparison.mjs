// The syntax runners' optional comparison settings: each option restores fields
// the structural comparison otherwise omits.
const SETTINGS = {
  '--positions': ['pos', 'end'],
  '--flags': ['flags'],
  '--scanner-hints': ['hasExtendedUnicodeEscape', 'numericLiteralFlags', 'rawText', 'templateFlags'],
};
export const COMPARISON_OPTIONS = Object.keys(SETTINGS);

// Compare serializable syntax, leaving compiler caches and binding data out.
// `flags` is the set of flags parsed by runnerArguments (corpus-selection.mjs);
// only comparison options are read. The returned function's `compared` lists
// the active settings by name, in COMPARISON_OPTIONS order.
export function normalizer(flags = new Set()) {
  if (!(flags instanceof Set)) throw new TypeError('normalizer takes the flag set parsed by runnerArguments');
  const omit = new Set(['id', 'flags', 'modifierFlagsCache', 'transformFlags', 'parent',
    'original', 'emitNode', 'symbol', 'localSymbol', 'locals', 'nextContainer', 'flowNode',
    'hasExtendedUnicodeEscape', 'numericLiteralFlags', 'multiLine', 'jsDoc', 'pos', 'end',
    'fileName', 'path', 'resolvedPath', 'originalFileName', 'languageVersion', 'languageVariant',
    'scriptKind', 'isDeclarationFile', 'hasNoDefaultLib', 'nodeCount', 'identifierCount',
    'symbolCount', 'parseDiagnostics', 'bindDiagnostics', 'pragmas', 'referencedFiles',
    'typeReferenceDirectives', 'libReferenceDirectives', 'amdDependencies', 'identifiers',
    'jsDocParsingMode', 'externalModuleIndicator', 'endOfFileToken', 'textSourceNode',
    'internalFlags', 'rawText', 'templateFlags', 'commentDirectives', 'checkJsDirective', 'moduleName']);
  const active = COMPARISON_OPTIONS.filter(option => flags.has(option));
  for (const option of active) for (const key of SETTINGS[option]) omit.delete(key);
  function normalize(value, root = false) {
    if (!value || typeof value !== 'object') return value;
    const result = Array.isArray(value) ? [] : {};
    const pending = [{ value, target: result, root }];
    while (pending.length) {
      const { value, target, root } = pending.pop();
      const keys = Array.isArray(value) ? Array.from({ length: value.length }, (_, i) => i)
        : Object.keys(value).sort().filter(key => !omit.has(key) && !(root && key === 'text')
          && value[key] !== undefined && typeof value[key] !== 'function');
      for (const key of keys) {
        const child = value[key];
        if (key === 'flags') target[key] = child & ~2097152; // HasAggregatedChildData is a lazy compiler cache.
        else if (!child || typeof child !== 'object') target[key] = child;
        else {
          target[key] = Array.isArray(child) ? [] : {};
          pending.push({ value: child, target: target[key], root: false });
        }
      }
    }
    return result;
  }
  normalize.compared = active.map(option => option.slice(2));
  return normalize;
}

// What runners record as `compared`: the active settings, 'structure only'
// without any, or 'acceptance only' when no trees are compared (upstream
// --check-only).
export function describeComparison(compared, { trees = true } = {}) {
  if (!trees) return 'acceptance only';
  return compared.length ? compared.join(', ') : 'structure only';
}

export function firstDifference(actual, expected, path = '$') {
  const pending = [{ actual, expected, path }];
  while (pending.length) {
    const { actual, expected, path, actualPresent = true, expectedPresent = true } = pending.pop();
    if (actualPresent !== expectedPresent) {
      return { path, actual, expected, actualMissing: !actualPresent, expectedMissing: !expectedPresent };
    }
    if (actual === expected) continue;
    if (!actual || !expected || typeof actual !== 'object' || typeof expected !== 'object') {
      return { path, actual, expected };
    }
    const array = Array.isArray(actual);
    if (array !== Array.isArray(expected)) return { path, actual, expected };
    if (array && actual.length !== expected.length) {
      return { path: `${path}.length`, actual: actual.length, expected: expected.length };
    }
    const keys = array ? Array.from({ length: actual.length }, (_, index) => index)
      : [...new Set([...Object.keys(actual), ...Object.keys(expected)])].sort();
    for (let i = keys.length - 1; i >= 0; i--) {
      const key = keys[i];
      const actualPresent = Object.hasOwn(actual, key);
      const expectedPresent = Object.hasOwn(expected, key);
      pending.push({ actual: actualPresent ? actual[key] : undefined,
        expected: expectedPresent ? expected[key] : undefined, path: `${path}.${key}`,
        actualPresent, expectedPresent });
    }
  }
}
