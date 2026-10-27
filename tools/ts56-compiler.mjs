// The pinned TypeScript 5.6.3 compiler behind the TypeScript 5.6 dialect's
// tools, such as `ts56-oracle.mjs` and `program-inputs.mjs --oracle=5.6`. It
// never replaces the 6.0.3 syntax oracle or the 7.0.2 semantic oracle.
//
//   TYPE_SCRIPT_56_API=/path/to/typescript-5.6.3/package
//
// The variable names the package directory, the one holding package.json and
// lib/; unset, the package is looked up at
// `.cache/compiler-oracles/typescript-5.6.3/package` in the canonical checkout
// of this repository, found from any linked worktree through `git rev-parse
// --git-common-dir`. The package must say version 5.6.3. Nothing is downloaded.
//
// 5.6.3 writes `.tsbuildinfo` for a configuration with `incremental: true`, so
// every program here is built with `incremental` forced false: `parseConfig`
// overrides it, and a `tsc` run must pass `--incremental false` (`TSC_ARGUMENTS`).
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const TS56_VERSION = '5.6.3';
export const TS56_VARIABLE = 'TYPE_SCRIPT_56_API';
// Arguments every 5.6.3 `tsc` run takes after its own.
export const TSC_ARGUMENTS = ['--incremental', 'false'];

const TOOLS = fileURLToPath(new URL('.', import.meta.url));

// Where the package should be, `{ packageDir }`, with `missing` stating why
// when no package.json is there or no checkout was found.
function find(variable) {
  if (variable) {
    const packageDir = path.resolve(variable);
    return fs.existsSync(path.join(packageDir, 'package.json')) ? { packageDir }
      : { packageDir, missing: `${TS56_VARIABLE} names ${packageDir}, which holds no package.json` };
  }
  const git = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { cwd: TOOLS, encoding: 'utf8' });
  const common = git.status === 0 ? git.stdout.trim() : '';
  if (!common) return { missing: `${TS56_VARIABLE} is unset and no Git checkout was found` };
  const packageDir = path.join(path.dirname(common), '.cache/compiler-oracles/typescript-5.6.3/package');
  return fs.existsSync(path.join(packageDir, 'package.json')) ? { packageDir }
    : { packageDir, missing: `${TS56_VARIABLE} is unset and no TypeScript ${TS56_VERSION} package is at ${packageDir}` };
}

// The 5.6.3 package, `{ packageDir }`, from `variable` (default
// TYPE_SCRIPT_56_API) or the canonical checkout. Throws, stating why, when the
// package is missing or its package.json does not say 5.6.3.
export function locateTypeScript56(variable = process.env[TS56_VARIABLE]) {
  const { packageDir, missing } = find(variable);
  if (missing) throw new Error(missing);
  let version;
  try { ({ version } = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'))); }
  catch (error) { throw new Error(`${packageDir}/package.json is not readable: ${error.message}`); }
  if (version !== TS56_VERSION) throw new Error(`expected TypeScript ${TS56_VERSION} at ${packageDir}, found version ${version}`);
  return { packageDir };
}

// Why a harness test cannot find the 5.6.3 package, or false. A package that
// is present but not 5.6.3 is not missing: its tests run and fail.
export function typeScript56Unavailable(variable = process.env[TS56_VARIABLE]) {
  return find(variable).missing ?? false;
}

// The located compiler: its API module, `lib/tsc.js`, and the canonical
// directory its standard libraries are read from. Throws, stating why, when
// the package is missing or is not 5.6.3.
export function loadTypeScript56(located = locateTypeScript56()) {
  const { packageDir } = located;
  const tsc = path.join(packageDir, 'lib/tsc.js');
  if (!fs.existsSync(tsc)) throw new Error(`TypeScript ${TS56_VERSION} at ${packageDir} has no lib/tsc.js`);
  const ts = createRequire(import.meta.url)(path.join(packageDir, 'lib/typescript.js'));
  if (ts.version !== TS56_VERSION) throw new Error(`${packageDir}/lib/typescript.js is version ${ts.version}, not ${TS56_VERSION}`);
  return { ts, packageDir, tsc, libraries: fs.realpathSync(path.join(packageDir, 'lib')) };
}

// The configuration as 5.6.3 parses it, with `incremental` forced false.
// Throws on any error the parse reports, such as an unreadable file or no inputs.
export function parseConfig(ts, config) {
  const errors = [];
  const host = { ...ts.sys, onUnRecoverableConfigFileDiagnostic: diagnostic => errors.push(diagnostic) };
  const parsed = ts.getParsedCommandLineOfConfigFile(config, { incremental: false }, host);
  errors.push(...(parsed?.errors ?? []).filter(diagnostic => diagnostic.category === ts.DiagnosticCategory.Error));
  if (errors.length || !parsed) {
    const messages = errors.map(diagnostic => `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ')}`);
    throw new Error(`TypeScript ${TS56_VERSION} rejects ${config}: ${messages.join('; ') || 'no configuration'}`);
  }
  return parsed;
}

// How the program listings name a compiler's file: a standard library from
// the compiler's own directory `libraries` by its file name, any other file
// by its canonical path relative to `root`, `/`-separated.
export function programFileName(root, libraries) {
  return fileName => {
    const real = fs.realpathSync(fileName);
    if (path.dirname(real) === libraries) return path.basename(real);
    return path.relative(root, real).split(path.sep).join('/');
  };
}
