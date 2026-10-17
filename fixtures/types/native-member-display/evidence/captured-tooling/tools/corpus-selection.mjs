// Parse a runner's command line against the options it reads. `options` lists
// each spelling: a flag such as '--check-only', or a valued option written with
// its '=', such as '--limit='. Any other argument starting with '-' is rejected,
// as is a repeated valued option or more than `positional` other arguments: an
// option the runner ignores would otherwise compare or select less than the
// command asked for while reporting the same pass.
export function runnerArguments(args, { options = [], positional = 0 } = {}) {
  const flags = new Set(), values = new Map(), paths = [];
  for (const arg of args) {
    if (!arg.startsWith('-') || arg === '-') { paths.push(arg); continue; }
    const equals = arg.indexOf('=');
    const name = equals < 0 ? arg : arg.slice(0, equals + 1);
    if (!options.includes(name))
      throw new Error(`Unknown option ${arg}; this runner accepts ${options.length ? options.join(' ') : 'no options'}`);
    if (equals < 0) flags.add(name);
    else if (values.has(name)) throw new Error(`${name.slice(0, -1)} is given more than once`);
    else values.set(name, arg.slice(equals + 1));
  }
  if (paths.length > positional) throw new Error(`Unexpected argument ${paths[positional]}`);
  return { flags, values, paths };
}

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
