// Semantic conformance: TypeScript 7.0.2 evidence for the Rust reference tool.
//
//   node tools/semantic-conformance.mjs markers [--limit=N] [--report=FILE] [PATH ...]
//   node tools/semantic-conformance.mjs compare [--limit=N] [--report=FILE] [PATH ...]
//
// A PATH is a fixture directory containing tsconfig.json, or a configuration
// file. Without paths, every directory under fixtures/references is used.
//
// markers: checks that each fixture's comment markers (the syntax of
// tests/support/mod.rs) agree with the normalized semantic oracle. Every
// identifier token must be marked; each label must denote exactly the tokens
// of one oracle group; `!` must be a member or non-reference position and `?`
// an unresolved target; marked roles must match the tokens' syntactic roles.
// Standard library declarations appear in groups unmarked, at their vendored
// copies. --limit selects the first N fixtures.
//
// compare: queries the Rust tool at every oracle token through one
// `reference_batch` process per project and reports per-token differences:
// classification (expected groups, UnsupportedTarget, or UnresolvedTarget),
// missing/extra groups and occurrences, occurrence names and roles,
// declaration sets, merged and split groups, panics, and oracle anomalies
// (recorded, never filtered). It also issues one name query per distinct
// spelling among the queried tokens' name candidates and compares it with the
// union of the candidates' groups, or with the error a failing candidate
// implies. --limit bounds the number of queried tokens across all projects.
// The summary counts queried tokens by expected outcome and by normalization
// code (see semantic-oracle.mjs), and name queries separately.
//
// Environment: TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE select the
// pinned compiler (see semantic-oracle.mjs). REFERENCE_BATCH names the example
// binary, by default target/debug/examples/reference_batch; build it with
// `cargo build --offline --example reference_batch`.
//
// Reports: .cache/semantic-markers.json or .cache/semantic-conformance.json.
// The exit status is 1 for any difference, invalid fixture, or setup failure.
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { runnerArguments, corpusLimit, requireComparisons } from './corpus-selection.mjs';
import {
  SemanticSession, loadSemanticApi, normalizeOracle, lineStarts, position, locationKey, describeLocation, compareLocations,
} from './semantic-oracle.mjs';

const SAMPLE = 20;
const ROLE_LETTERS = { d: 'declaration', u: 'use', i: 'import', e: 'export' };

// ---------------------------------------------------------------------------
// Markers

const IDENTIFIER_CHARACTER = /^[\p{Alphabetic}\p{N}_$‌‍]$/u;

// Mirrors tests/support/mod.rs so both sides read the same expectations,
// including a leading `#` for private names.
function identifierLength(text, start) {
  let index = start + (text[start] === '#' ? 1 : 0);
  for (;;) {
    if (text.startsWith('\\u{', index)) {
      const close = text.indexOf('}', index + 3);
      if (close < 0) throw new Error('unterminated \\u{ escape');
      index = close + 1;
    } else if (text.startsWith('\\u', index)) {
      index += 6;
    } else {
      const code = text.codePointAt(index);
      if (code === undefined || !IDENTIFIER_CHARACTER.test(String.fromCodePoint(code))) return index - start;
      index += code > 0xffff ? 2 : 1;
    }
  }
}

function decodeIdentifier(raw) {
  return raw.replace(/\\u\{([0-9a-fA-F]+)\}|\\u([0-9a-fA-F]{4})/g, (_, braced, plain) => String.fromCodePoint(parseInt(braced ?? plain, 16)));
}

export function parseMarkers(text, file) {
  const markers = [];
  const starts = lineStarts(text);
  let search = 0;
  for (;;) {
    const open = text.indexOf('/*@', search);
    if (open < 0) return markers;
    const close = text.indexOf('*/', open);
    if (close < 0) throw new Error(`${file}: unclosed marker at ${describeOffset(starts, open)}`);
    const spec = text.slice(open + 3, close);
    const start = close + 2;
    const end = start + identifierLength(text, start);
    if (end === start) throw new Error(`${file}: marker at ${describeOffset(starts, open)} precedes no identifier`);
    let expect;
    if (spec === '!') expect = { kind: 'unsupported' };
    else if (spec === '?') expect = { kind: 'unresolved' };
    else {
      const colon = spec.indexOf(':');
      const labels = (colon < 0 ? spec : spec.slice(0, colon)).split(',');
      if (labels.some(label => !label)) throw new Error(`${file}: empty marker label at ${describeOffset(starts, open)}`);
      let roles = null;
      if (colon >= 0) {
        roles = [...new Set([...spec.slice(colon + 1)].map(letter => {
          if (!ROLE_LETTERS[letter]) throw new Error(`${file}: unknown role ${letter} at ${describeOffset(starts, open)}`);
          return ROLE_LETTERS[letter];
        }))].sort();
      }
      expect = { kind: 'groups', labels, roles };
    }
    markers.push({ location: { file, start: position(starts, start), end: position(starts, end) }, name: decodeIdentifier(text.slice(start, end)), expect });
    search = start;
  }
}

function describeOffset(starts, offset) {
  const { line, column } = position(starts, offset);
  return `${line}:${column}`;
}

export function collectMarkers(root) {
  const markers = [];
  const visit = directory => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (/\.(ts|tsx|mts|cts)$/.test(entry.name))
        markers.push(...parseMarkers(fs.readFileSync(file, 'utf8'), path.relative(root, file).split(path.sep).join('/')));
    }
  };
  visit(root);
  return markers.sort((a, b) => compareLocations(a.location, b.location));
}

const sameSet = (a, b) => a.size === b.size && [...a].every(item => b.has(item));

// Problems that make the markers untrustworthy as evidence; empty when they agree.
export function checkMarkers(oracle, markers) {
  const problems = [];
  const problem = (kind, location, message, details = {}) => problems.push({ kind, location: location && describeLocation(location), message, ...details });
  const tokens = new Map(oracle.tokens.map(token => [locationKey(token.location), token]));
  const marked = new Map();
  for (const marker of markers) {
    const key = locationKey(marker.location);
    const token = tokens.get(key);
    if (marked.has(key)) problem('duplicate-marker', marker.location, `\`${marker.name}\` is marked twice`);
    marked.set(key, marker);
    if (!token) { problem('no-token', marker.location, `marker precedes \`${marker.name}\`, which TypeScript does not report as an identifier token`); continue; }
    if (describeLocation(token.location) !== describeLocation(marker.location) || token.name !== marker.name)
      problem('token-text', marker.location, `marker reads \`${marker.name}\` at ${describeLocation(marker.location)}; TypeScript reads \`${token.name}\` at ${describeLocation(token.location)}`);
    const wanted = marker.expect.kind === 'groups' ? 'groups' : marker.expect.kind;
    if (token.outcome !== wanted) {
      problem('classification', marker.location, `\`${marker.name}\` is marked ${wanted}, but TypeScript expects ${token.outcome} (${describeToken(token)})`);
      continue;
    }
    if (marker.expect.roles && marker.expect.roles.join() !== [...token.roles].sort().join())
      problem('roles', marker.location, `\`${marker.name}\` is marked ${marker.expect.roles.join('+')}, but its syntactic roles are ${[...token.roles].sort().join('+') || 'none'}`);
  }
  for (const token of oracle.tokens)
    if (!marked.has(locationKey(token.location))) problem('unmarked', token.location, `\`${token.name}\` is not marked (TypeScript: ${describeToken(token)})`);
  // Each label must denote exactly the tokens of one distinct oracle group.
  const byLabel = new Map();
  for (const marker of markers)
    if (marker.expect.kind === 'groups')
      for (const label of marker.expect.labels) {
        if (!byLabel.has(label)) byLabel.set(label, new Set());
        byLabel.get(label).add(locationKey(marker.location));
      }
  const byGroup = new Map();
  for (const token of oracle.tokens)
    for (const group of token.groups) {
      if (!byGroup.has(group)) byGroup.set(group, new Set());
      byGroup.get(group).add(locationKey(token.location));
    }
  const owner = new Map();
  for (const [label, keys] of byLabel) {
    const candidates = [...byGroup].filter(([, groupKeys]) => sameSet(keys, groupKeys)).map(([group]) => group);
    const group = candidates.find(candidate => !owner.has(candidate)) ?? candidates[0];
    if (group === undefined) {
      const [best] = [...byGroup].map(([index, groupKeys]) => [index, [...keys].filter(key => groupKeys.has(key)).length])
        .sort((a, b) => b[1] - a[1]);
      const groupKeys = best && best[1] ? byGroup.get(best[0]) : new Set();
      const unmarked = [...groupKeys].filter(key => !keys.has(key)), extra = [...keys].filter(key => !groupKeys.has(key));
      problem('label', null, `label \`${label}\` matches no TypeScript group${best && best[1] ? ` (closest: ${describeGroup(oracle, best[0])})` : ''}`
        + `${unmarked.length ? `; group tokens without the label: ${unmarked.join(' ')}` : ''}${extra.length ? `; labelled tokens outside the group: ${extra.join(' ')}` : ''}`, { unmarked, extra });
    } else if (owner.has(group)) {
      problem('label', null, `labels \`${owner.get(group)}\` and \`${label}\` both denote ${describeGroup(oracle, group)}`);
    } else owner.set(group, label);
  }
  for (const group of byGroup.keys())
    if (!owner.has(group) && [...byGroup.get(group)].every(key => marked.get(key)?.expect.kind === 'groups'))
      problem('label', null, `no label denotes ${describeGroup(oracle, group)}`);
  for (const anomaly of oracle.anomalies) problem('anomaly', anomaly.location, `TypeScript and normalization disagree on \`${anomaly.name}\`: ${anomaly.reason}`);
  return problems;
}

function describeToken(token) {
  const parts = [token.classification];
  if (token.syntax) parts.push(token.syntax);
  if (token.groups.length) parts.push(`groups ${token.groups.join(',')}`);
  return parts.join(', ');
}

function describeGroup(oracle, index) {
  const group = oracle.groups[index];
  return `group ${index} (${group.declarations.map(declaration => `${declaration.name}@${locationKey(declaration.location)}`).join(', ')})`;
}

// ---------------------------------------------------------------------------
// Tool comparison

const EXPECTED_RESPONSE = { groups: 'groups', unsupported: 'UnsupportedTarget', unresolved: 'UnresolvedTarget' };

function sample(keys) {
  const list = [...keys].sort();
  return list.length > SAMPLE ? { count: list.length, locations: list.slice(0, SAMPLE) } : { count: list.length, locations: list };
}

// Differences between the oracle's expectation for `token` and one
// reference_batch response line.
export function compareResponse(oracle, token, response) {
  if (token.outcome === 'anomaly') return [{ kind: 'oracle-anomaly', message: oracle.anomalies.find(a => locationKey(a.location) === locationKey(token.location))?.reason }];
  if (!response || typeof response !== 'object') return [{ kind: 'invalid-response', message: JSON.stringify(response) }];
  if (typeof response.panic === 'string') return [{ kind: 'panic', message: response.panic }];
  const actual = response.error && typeof response.error.kind === 'string' ? response.error.kind
    : response.result && Array.isArray(response.result.groups) ? 'groups' : undefined;
  if (actual === undefined) return [{ kind: 'invalid-response', message: JSON.stringify(response).slice(0, 500) }];
  const expected = EXPECTED_RESPONSE[token.outcome];
  if (actual !== expected)
    return [{ kind: 'classification', expected, actual, typescript: token.classification, ...(token.syntax ? { syntax: token.syntax } : {}), ...(response.error ? { message: response.error.message } : {}) }];
  if (actual !== 'groups') return [];
  return compareGroups(token.groups.map(index => ({ index, ...oracle.groups[index] })), response.result.groups);
}

// One name query per distinct spelling among `tokens`' name candidates. Its
// candidates are every candidate token of that spelling in the project.
export function nameQueries(oracle, tokens = oracle.tokens) {
  const names = [...new Set(tokens.filter(token => token.candidate).map(token => token.name))].sort();
  return names.map(name => ({ name, candidates: oracle.tokens.filter(token => token.candidate && token.name === name) }));
}

// Differences between the expectation for a name query and one response. A
// name query returns the union of its candidates' groups, or fails as a
// position query on its first unsupported or unresolved candidate would. The
// tool's candidate order is not part of the contract, so the error may name
// any failing candidate whose outcome matches its kind.
export function compareNameResponse(oracle, query, response) {
  const anomalous = query.candidates.find(token => token.outcome === 'anomaly');
  if (anomalous) return [{ kind: 'oracle-anomaly', message: `candidate ${describeLocation(anomalous.location)} has no expected outcome` }];
  if (!response || typeof response !== 'object') return [{ kind: 'invalid-response', message: JSON.stringify(response) }];
  if (typeof response.panic === 'string') return [{ kind: 'panic', message: response.panic }];
  const actual = response.error && typeof response.error.kind === 'string' ? response.error.kind
    : response.result && Array.isArray(response.result.groups) ? 'groups' : undefined;
  if (actual === undefined) return [{ kind: 'invalid-response', message: JSON.stringify(response).slice(0, 500) }];
  const failing = query.candidates.filter(token => token.outcome !== 'groups');
  const describe = tokens => tokens.slice(0, SAMPLE).map(token => `${locationKey(token.location)} ${token.outcome}`);
  if (failing.length) {
    const expected = [...new Set(failing.map(token => EXPECTED_RESPONSE[token.outcome]))].sort();
    if (!expected.includes(actual))
      return [{ kind: 'classification', expected: expected.join('|'), actual, candidates: describe(failing), ...(response.error ? { message: response.error.message } : {}) }];
    const at = errorLocation(path.dirname(oracle.project), response.error);
    if (!failing.some(token => EXPECTED_RESPONSE[token.outcome] === actual && locationKey(token.location) === at))
      return [{ kind: 'error-location', expected: describe(failing), actual: at ?? 'none', message: response.error.message }];
    return [];
  }
  if (actual !== 'groups') return [{ kind: 'classification', expected: 'groups', actual, message: response.error.message }];
  const indices = [...new Set(query.candidates.flatMap(token => token.groups))].sort((a, b) => a - b);
  return compareGroups(indices.map(index => ({ index, ...oracle.groups[index] })), response.result.groups);
}

// The `file:line:column` key of a query error's location relative to the
// project directory, or undefined when the error has none.
function errorLocation(root, error) {
  if (typeof error.path !== 'string' || !error.position) return undefined;
  let file = path.resolve(root, error.path);
  try { file = fs.realpathSync(file); } catch { /* keep the reported spelling */ }
  return `${path.relative(root, file).split(path.sep).join('/')}:${error.position.line}:${error.position.column}`;
}

export function compareGroups(expected, actual) {
  const differences = [];
  // Each group lists a location once; set-based matching below would hide a
  // repeated one, so report repetitions first.
  for (const [index, group] of actual.entries()) {
    for (const [kind, items] of [['duplicate-occurrence', group.occurrences], ['duplicate-declaration', group.declarations]]) {
      const seen = new Map();
      for (const item of items) {
        const key = locationKey(item.location);
        seen.set(key, (seen.get(key) ?? 0) + 1);
      }
      const repeated = [...seen].filter(([, count]) => count > 1);
      if (repeated.length)
        differences.push({ kind, group: index, count: repeated.length, locations: repeated.slice(0, 20).map(([key, count]) => `${key} x${count}`) });
    }
  }
  const occurrenceKeys = group => new Map(group.occurrences.map(occurrence => [locationKey(occurrence.location), occurrence.name]));
  // Sorted roles joined by `+`, or null when an occurrence has no role list.
  const roleKeys = group => new Map(group.occurrences.map(occurrence =>
    [locationKey(occurrence.location), Array.isArray(occurrence.roles) ? [...occurrence.roles].sort().join('+') : null]));
  const declarationKeys = group => new Set(group.declarations.map(declaration => `${declaration.name}@${locationKey(declaration.location)}`));
  const declarationPlaces = group => new Set(group.declarations.map(declaration => locationKey(declaration.location)));
  const summarize = groups => {
    const sides = groups.map(group => ({ occurrences: occurrenceKeys(group), roles: roleKeys(group), declarations: declarationKeys(group), places: declarationPlaces(group) }));
    // A token in several groups (such as `export { Name }` for a type and a
    // value) says nothing about which groups correspond; match on the others.
    // Neither does a declaration in several groups, such as a pattern ambient
    // module's that a one-way augmentation merge also lists.
    const exclusive = keys => {
      const counts = new Map();
      for (const side of sides) for (const key of keys(side)) counts.set(key, (counts.get(key) ?? 0) + 1);
      return side => new Set([...keys(side)].filter(key => counts.get(key) === 1));
    };
    const exclusiveOccurrences = exclusive(side => side.occurrences.keys());
    const exclusivePlaces = exclusive(side => side.places);
    for (const side of sides) Object.assign(side, { exclusive: exclusiveOccurrences(side), exclusivePlaces: exclusivePlaces(side) });
    return sides;
  };
  const wanted = summarize(expected), found = summarize(actual);
  const strong = (a, b) => [...a.exclusivePlaces].some(key => b.exclusivePlaces.has(key)) || [...a.exclusive].some(key => b.exclusive.has(key));
  const weak = (a, b) => [...a.occurrences.keys()].some(key => b.occurrences.has(key));
  const related = (side, others) => {
    const indices = others.map((other, index) => (strong(side, other) ? index : -1)).filter(index => index >= 0);
    return indices.length ? indices : others.map((other, index) => (weak(side, other) ? index : -1)).filter(index => index >= 0);
  };
  const matches = wanted.map(side => related(side, found));
  const owners = found.map(side => related(side, wanted));
  const describe = group => group.declarations.map(declaration => `${declaration.name}@${locationKey(declaration.location)}`);
  wanted.forEach((group, index) => {
    const label = { group: describe(expected[index]) };
    if (!matches[index].length) {
      differences.push({ kind: 'missing-group', ...label, ...sample(group.occurrences.keys()) });
      return;
    }
    if (matches[index].length > 1) differences.push({ kind: 'split-group', ...label, into: matches[index].map(other => describe(actual[other])) });
    const present = new Map(matches[index].flatMap(other => [...found[other].occurrences]));
    const missing = [...group.occurrences.keys()].filter(key => !present.has(key));
    if (missing.length) differences.push({ kind: 'missing-occurrence', ...label, ...sample(missing) });
    const renamed = [...group.occurrences].filter(([key, name]) => present.has(key) && present.get(key) !== name);
    if (renamed.length) differences.push({ kind: 'occurrence-name', ...label, ...sample(renamed.map(([key, name]) => `${key} ${name} -> ${present.get(key)}`)) });
    // Roles are syntactic, so a token has the same roles in every group.
    const presentRoles = new Map(matches[index].flatMap(other => [...found[other].roles]));
    const reroled = [...group.roles].filter(([key, roles]) => roles !== null && presentRoles.has(key) && presentRoles.get(key) !== roles);
    const spell = roles => (roles === null ? 'missing' : roles || 'none');
    if (reroled.length) differences.push({ kind: 'occurrence-roles', ...label, ...sample(reroled.map(([key, roles]) => `${key} ${spell(roles)} -> ${spell(presentRoles.get(key))}`)) });
    const declarations = new Set(matches[index].flatMap(other => [...found[other].declarations]));
    const missingDeclarations = [...group.declarations].filter(key => !declarations.has(key));
    const extraDeclarations = [...declarations].filter(key => !group.declarations.has(key));
    if (missingDeclarations.length) differences.push({ kind: 'missing-declaration', ...label, ...sample(missingDeclarations) });
    if (extraDeclarations.length) differences.push({ kind: 'extra-declaration', ...label, ...sample(extraDeclarations) });
  });
  found.forEach((group, index) => {
    const label = { actual: describe(actual[index]) };
    if (!owners[index].length) {
      differences.push({ kind: 'extra-group', ...label, ...sample(group.occurrences.keys()) });
      return;
    }
    if (owners[index].length > 1) differences.push({ kind: 'merged-groups', ...label, groups: owners[index].map(other => describe(expected[other])) });
    const allowed = new Set(owners[index].flatMap(other => [...wanted[other].occurrences.keys()]));
    const extra = [...group.occurrences.keys()].filter(key => !allowed.has(key));
    if (extra.length) differences.push({ kind: 'extra-occurrence', ...label, ...sample(extra) });
  });
  return differences;
}

// Runs one reference_batch process. Responses are handled as they arrive, so
// large projects are never held in memory as a whole.
// The default timeout allows 10 minutes plus 5 ms per query: large projects
// return groups with thousands of occurrences for many of their tokens.
export function runBatch(binary, config, queries, onResponse, { timeout = 600_000 + 5 * queries.length } = {}) {
  return new Promise(resolve => {
    let child;
    try {
      child = spawn(binary, ['--project', config], { stdio: ['pipe', 'pipe', 'pipe'] });
    } catch (error) {
      resolve({ error: error.message, responses: 0 });
      return;
    }
    let responses = 0, projectError, stderr = '', failure, settled = false;
    const timer = setTimeout(() => { failure = `timed out after ${timeout} ms`; child.kill('SIGKILL'); }, timeout);
    child.on('error', error => { failure ??= error.message; });
    child.stdin.on('error', () => { /* the process exited early; its status explains why */ });
    child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-65_536); });
    const lines = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
    lines.on('line', line => {
      let response;
      try { response = JSON.parse(line); } catch { failure ??= `invalid JSON line: ${line.slice(0, 200)}`; return; }
      if (response && response.projectError && responses === 0) { projectError = response.projectError; return; }
      if (responses >= queries.length) { failure ??= 'more responses than queries'; return; }
      onResponse(responses, response);
      responses++;
    });
    // Settle once the process has exited and every output line was handled.
    let exit, linesClosed = false;
    const finish = () => {
      if (settled || !exit || !linesClosed) return;
      settled = true;
      clearTimeout(timer);
      const { status, signal } = exit;
      if (!projectError && !failure && (status !== 0 || signal)) failure = `exit ${status}, signal ${signal}${stderr.trim() ? `: ${stderr.trim().slice(-2000)}` : ''}`;
      if (!projectError && !failure && responses !== queries.length) failure = `answered ${responses} of ${queries.length} queries`;
      resolve({ status, signal, responses, projectError, error: failure });
    };
    lines.on('close', () => { linesClosed = true; finish(); });
    child.on('close', (status, signal) => { exit = { status, signal }; finish(); });
    child.stdin.end(queries.map(query => JSON.stringify(query)).join('\n') + (queries.length ? '\n' : ''));
  });
}

// ---------------------------------------------------------------------------
// CLI

export function projectConfigs(args, defaultRoot = 'fixtures/references') {
  const paths = args.filter(arg => !arg.startsWith('--'));
  if (!paths.length) {
    if (!fs.existsSync(defaultRoot)) throw new Error(`No fixtures under ${defaultRoot}`);
    paths.push(...fs.readdirSync(defaultRoot, { withFileTypes: true }).filter(entry => entry.isDirectory())
      .map(entry => path.join(defaultRoot, entry.name)).sort());
  }
  return paths.map(item => {
    const config = fs.existsSync(item) && fs.statSync(item).isDirectory() ? path.join(item, 'tsconfig.json') : item;
    if (!fs.existsSync(config)) throw new Error(`No configuration at ${config}`);
    return path.resolve(config);
  });
}

function writeReport(file, report) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(report, null, 2));
}

async function markersMode(args, semantic) {
  const configs = projectConfigs(args).slice(0, corpusLimit(args));
  requireComparisons(configs.length);
  const session = new SemanticSession(semantic);
  const fixtures = [];
  try {
    for (const config of configs) {
      const fixture = path.relative(process.cwd(), path.dirname(config)) || '.';
      try {
        const oracle = normalizeOracle(session.collect(config));
        const markers = collectMarkers(path.dirname(config));
        if (!markers.length) throw new Error('fixture has no markers');
        const problems = checkMarkers(oracle, markers);
        fixtures.push({ fixture, markers: markers.length, tokens: oracle.tokens.length, groups: oracle.groups.length,
          diagnostics: oracle.diagnostics.map(d => `${d.file ?? ''}${d.start ? `:${d.start.line}:${d.start.column}` : ''} TS${d.code} ${d.message}`), problems });
      } catch (error) {
        fixtures.push({ fixture, problems: [{ kind: 'setup', message: error.message }] });
      }
    }
  } finally {
    session.close();
  }
  for (const { fixture, problems } of fixtures)
    for (const item of problems) console.log(`FAIL ${fixture}: ${item.kind}${item.location ? ` ${item.location}` : ''}: ${item.message}`);
  const failed = fixtures.filter(fixture => fixture.problems.length).length;
  const report = { compiler: semantic.version, mode: 'markers', fixtures: fixtures.length, passed: fixtures.length - failed, failed,
    markers: fixtures.reduce((sum, fixture) => sum + (fixture.markers ?? 0), 0), results: fixtures };
  writeReport(reportPath(args, '.cache/semantic-markers.json'), report);
  const { results, ...summary } = report;
  console.log(JSON.stringify(summary));
  return failed ? 1 : 0;
}

async function compareMode(args, semantic) {
  const configs = projectConfigs(args);
  let remaining = corpusLimit(args);
  const binary = path.resolve(process.env.REFERENCE_BATCH ?? 'target/debug/examples/reference_batch');
  if (!fs.existsSync(binary)) throw new Error(`No reference_batch binary at ${binary}; run \`cargo build --offline --example reference_batch\` or set REFERENCE_BATCH`);
  const session = new SemanticSession(semantic);
  const projects = [], failures = [], byKind = {};
  const names = { queried: 0, passed: 0, failed: 0, byKind: {} };
  // Expected outcomes of the queried tokens, and how many of them the oracle
  // moved away from TypeScript's resolution (see normalizationCode).
  const outcomes = {}, normalizations = {};
  const count = (counts, key) => { counts[key] = (counts[key] ?? 0) + 1; };
  let queried = 0, passed = 0;
  try {
    for (const config of configs) {
      if (remaining <= 0) break;
      const entry = { project: config };
      projects.push(entry);
      let oracle;
      try { oracle = normalizeOracle(session.collect(config, { diagnostics: false })); }
      catch (error) { entry.setupError = `oracle: ${error.message}`; continue; }
      const root = path.dirname(oracle.project);
      const tokens = oracle.tokens.slice(0, remaining);
      remaining -= tokens.length;
      const nameList = nameQueries(oracle, tokens);
      Object.assign(entry, { tokens: oracle.tokens.length, queried: tokens.length, groups: oracle.groups.length, anomalies: oracle.anomalies.length,
        passed: 0, failed: 0, names: nameList.length, namesPassed: 0, namesFailed: 0 });
      for (const token of tokens) {
        count(outcomes, token.outcome);
        if (token.normalization) count(normalizations, token.normalization);
      }
      const queries = [
        ...tokens.map(token => ({ at: { file: path.join(root, token.location.file), line: token.location.start.line, column: token.location.start.column } })),
        ...nameList.map(({ name }) => ({ name, file: null })),
      ];
      const run = await runBatch(binary, oracle.project, queries, (index, response) => {
        if (index >= tokens.length) {
          const query = nameList[index - tokens.length];
          names.queried++;
          const differences = compareNameResponse(oracle, query, response);
          if (!differences.length) { names.passed++; entry.namesPassed++; return; }
          names.failed++;
          entry.namesFailed++;
          for (const kind of new Set(differences.map(difference => difference.kind))) count(names.byKind, kind);
          failures.push({ project: config, query: 'name', name: query.name,
            candidates: query.candidates.map(token => `${describeLocation(token.location)} ${token.outcome}`).slice(0, SAMPLE), differences });
          return;
        }
        queried++;
        const token = tokens[index];
        const differences = compareResponse(oracle, token, response);
        if (!differences.length) { entry.passed++; return; }
        entry.failed++;
        for (const kind of new Set(differences.map(difference => difference.kind))) count(byKind, kind);
        failures.push({ project: config, query: 'at', location: describeLocation(token.location), name: token.name, typescript: token.classification,
          expected: token.outcome, ...(token.syntax ? { syntax: token.syntax } : {}),
          ...(token.normalization ? { normalization: token.normalization } : {}), differences });
      });
      passed += entry.passed;
      if (run.projectError) entry.setupError = `reference_batch project error ${run.projectError.kind}: ${run.projectError.message}`;
      else if (run.error) entry.setupError = `reference_batch: ${run.error}`;
    }
  } finally {
    session.close();
  }
  const setupFailures = projects.filter(project => project.setupError);
  for (const project of setupFailures) console.log(`SETUP ${project.project}: ${project.setupError}`);
  for (const failure of failures.slice(0, 50)) {
    const directory = path.relative(process.cwd(), path.dirname(failure.project)) || '.';
    const subject = failure.query === 'name' ? `name \`${failure.name}\` (${failure.candidates.length} candidates)`
      : `${failure.location} \`${failure.name}\` (${failure.expected})`;
    console.log(`FAIL ${directory}: ${subject}: ${failure.differences.map(d => d.kind).join(', ')}`);
  }
  if (failures.length > 50) console.log(`... ${failures.length - 50} more failures in the report`);
  if (!setupFailures.length) requireComparisons(queried);
  const report = { compiler: semantic.version, mode: 'compare', binary, projects, tokens: projects.reduce((sum, p) => sum + (p.tokens ?? 0), 0),
    queried, passed, failed: queried - passed, setupFailures: setupFailures.length, byKind, names,
    outcomes: sortedCounts(outcomes), normalizations: sortedCounts(normalizations), failures };
  writeReport(reportPath(args, '.cache/semantic-conformance.json'), report);
  const { failures: _, projects: __, ...summary } = report;
  console.log(JSON.stringify(summary));
  return failures.length || setupFailures.length ? 1 : 0;
}

const sortedCounts = counts => Object.fromEntries(Object.entries(counts).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));

function reportPath(args, fallback) {
  const option = args.find(arg => arg.startsWith('--report='));
  return path.resolve(option ? option.slice('--report='.length) : fallback);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const [mode, ...args] = process.argv.slice(2);
    if (mode !== 'markers' && mode !== 'compare') throw new Error('Usage: node tools/semantic-conformance.mjs markers|compare [--limit=N] [--report=FILE] [PATH ...]');
    runnerArguments(args, { options: ['--limit=', '--report='], positional: Infinity });
    corpusLimit(args);
    const semantic = await loadSemanticApi();
    process.exitCode = mode === 'markers' ? await markersMode(args, semantic) : await compareMode(args, semantic);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
