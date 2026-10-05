// An omitted limit means the complete corpus. Explicit limits must describe a
// nonempty, bounded selection rather than silently becoming zero or unlimited.
export function corpusLimit(args, fallback = Infinity, maximum = Infinity) {
  const option = args.find(arg => arg.startsWith('--limit='));
  if (option === undefined) return fallback;
  const limit = Number(option.slice(8));
  if (!Number.isSafeInteger(limit) || limit <= 0 || limit > maximum)
    throw new Error(`--limit must be a positive integer${Number.isFinite(maximum) ? ` no greater than ${maximum}` : ''}`);
  return limit;
}

export function requireComparisons(count) {
  if (!count) throw new Error('No cases were compared; check the corpus paths and selection');
}
