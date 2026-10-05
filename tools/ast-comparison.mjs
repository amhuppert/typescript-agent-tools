// Compare serializable syntax, leaving compiler caches and binding data out.
export function normalizer(args = []) {
  const omit = new Set(['id', 'flags', 'modifierFlagsCache', 'transformFlags', 'parent',
    'original', 'emitNode', 'symbol', 'localSymbol', 'locals', 'nextContainer', 'flowNode',
    'hasExtendedUnicodeEscape', 'numericLiteralFlags', 'multiLine', 'jsDoc', 'pos', 'end',
    'fileName', 'path', 'resolvedPath', 'originalFileName', 'languageVersion', 'languageVariant',
    'scriptKind', 'isDeclarationFile', 'hasNoDefaultLib', 'nodeCount', 'identifierCount',
    'symbolCount', 'parseDiagnostics', 'bindDiagnostics', 'pragmas', 'referencedFiles',
    'typeReferenceDirectives', 'libReferenceDirectives', 'amdDependencies', 'identifiers',
    'jsDocParsingMode', 'externalModuleIndicator', 'endOfFileToken', 'textSourceNode',
    'internalFlags', 'rawText', 'templateFlags', 'commentDirectives', 'checkJsDirective', 'moduleName']);
  if (args.includes('--positions')) { omit.delete('pos'); omit.delete('end'); }
  if (args.includes('--flags')) omit.delete('flags');
  if (args.includes('--scanner-hints')) {
    for (const key of ['hasExtendedUnicodeEscape', 'numericLiteralFlags', 'rawText', 'templateFlags']) omit.delete(key);
  }
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
  return normalize;
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
