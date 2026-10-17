// Vendor the pinned TypeScript 7.0.2 standard library declarations, or check
// the vendored copy against its sources without writing.
//
//   node tools/vendor-typescript-libs.mjs [--check]
//
// Sources, all read-only and never downloaded:
//   TYPE_SCRIPT_NATIVE      the native compiler, lib/tsc inside the platform
//                           package whose lib/ directory holds the declarations
//                           (default: the canonical checkout's
//                           .cache/compiler-oracles/typescript-<platform>-7.0.2)
//   the package archive     <package directory>.tgz beside the extracted package
//   TYPE_SCRIPT_GO_SOURCE   a typescript-go checkout at tag typescript/v7.0.2
//                           (default: .cache/typescript-go-7.0.2), whose
//                           internal/bundled/libs must match the package files and
//                           whose internal/tsoptions/enummaps.go supplies the
//                           library catalog recorded in provenance.json.
//
// The Rust catalog (src/references/project/libraries.rs) is checked against
// provenance.json by its unit tests; tools/vendor-typescript-libs.test.mjs
// checks provenance.json against the vendored files and the native compiler.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const VERSION = '7.0.2';
export const TAG = `typescript/v${VERSION}`;
export const REPOSITORY = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const VENDORED = path.join(REPOSITORY, 'vendor', `typescript-${VERSION}-lib`);
const LICENSE_FILES = ['LICENSE', 'NOTICE.txt'];

export const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const isLibrary = name => /^lib\..*\.d\.ts$/.test(name) || name === 'lib.d.ts';

function compilerCache() {
  const result = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { cwd: REPOSITORY, encoding: 'utf8' });
  if (result.status !== 0 || !result.stdout.trim()) throw new Error('no Git checkout found for the compiler cache');
  return path.join(path.dirname(result.stdout.trim()), '.cache');
}

export function sources() {
  const cache = () => compilerCache();
  const native = process.env.TYPE_SCRIPT_NATIVE
    ?? path.join(cache(), 'compiler-oracles', `typescript-${process.platform}-${process.arch}-${VERSION}`, 'package', 'lib', 'tsc');
  const libraries = path.dirname(path.resolve(native));
  const packageDir = path.dirname(libraries);
  const archive = `${path.dirname(packageDir)}.tgz`;
  const go = process.env.TYPE_SCRIPT_GO_SOURCE ?? path.join(cache(), `typescript-go-${VERSION}`);
  return { libraries, packageDir, archive, go };
}

// tsoptions.LibMap in declaration order and targetToLibMap, read from
// enummaps.go at the pinned tag.
export function catalogFromGo(text) {
  const block = (start) => {
    const at = text.indexOf(start);
    if (at < 0) throw new Error(`enummaps.go has no ${start}`);
    return text.slice(at, text.indexOf('\n})', at));
  };
  const libMap = [...block('var LibMap =').matchAll(/\{Key: "([^"]+)", Value: "([^"]+)"\}/g)].map(([, key, file]) => [key, file]);
  const targets = {};
  for (const [, target, file] of block('var targetToLibMap =').matchAll(/core\.ScriptTarget(\w+): "([^"]+)"/g))
    targets[target.toLowerCase()] = file;
  if (libMap.length !== 107) throw new Error(`expected 107 LibMap entries, found ${libMap.length}`);
  return { libMap, targets };
}

// The complete provenance record and the files it describes.
export function collect(source = sources()) {
  for (const [label, file] of [['package', path.join(source.packageDir, 'package.json')], ['archive', source.archive],
    ['typescript-go checkout', path.join(source.go, 'internal/tsoptions/enummaps.go')]])
    if (!fs.existsSync(file)) throw new Error(`no ${label} at ${file}`);
  const manifest = JSON.parse(fs.readFileSync(path.join(source.packageDir, 'package.json'), 'utf8'));
  if (manifest.version !== VERSION) throw new Error(`expected package version ${VERSION}, found ${manifest.version}`);
  const head = spawnSync('git', ['-C', source.go, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
  if (head !== manifest.gitHead) throw new Error(`typescript-go checkout is at ${head}, the package was built from ${manifest.gitHead}`);
  const names = fs.readdirSync(source.libraries).filter(isLibrary).sort();
  const bundled = path.join(source.go, 'internal/bundled/libs');
  const bundledNames = fs.readdirSync(bundled).filter(isLibrary).sort();
  if (JSON.stringify(names) !== JSON.stringify(bundledNames)) throw new Error('the package and internal/bundled/libs list different files');
  const files = new Map();
  for (const name of names) {
    const bytes = fs.readFileSync(path.join(source.libraries, name));
    if (!bytes.equals(fs.readFileSync(path.join(bundled, name)))) throw new Error(`${name} differs from internal/bundled/libs`);
    files.set(name, bytes);
  }
  for (const name of LICENSE_FILES) files.set(name, fs.readFileSync(path.join(source.packageDir, name)));
  const enummaps = fs.readFileSync(path.join(source.go, 'internal/tsoptions/enummaps.go'));
  const { libMap, targets } = catalogFromGo(enummaps.toString('utf8'));
  for (const [, file] of libMap) if (!files.has(file)) throw new Error(`LibMap names ${file}, which the package lacks`);
  const provenance = {
    package: manifest.name,
    version: manifest.version,
    license: manifest.license,
    archive: { file: path.basename(source.archive), sha256: sha256(fs.readFileSync(source.archive)) },
    upstream: {
      repository: 'https://github.com/microsoft/typescript-go',
      tag: TAG,
      commit: manifest.gitHead,
      libraries: 'internal/bundled/libs',
      catalog: 'internal/tsoptions/enummaps.go',
      catalogSha256: sha256(enummaps),
    },
    libMap,
    targetLibraries: targets,
    otherTargetLibrary: 'lib.d.ts',
    files: Object.fromEntries([...files].map(([name, bytes]) => [name, sha256(bytes)])),
  };
  return { provenance, files };
}

// One library entry per line.
export const provenanceText = provenance => JSON.stringify(provenance, null, 2)
  .replace(/\[\n\s+("[^"]*"),\n\s+("[^"]*")\n\s+\]/g, '[$1, $2]') + '\n';

// Differences between the vendored directory and the expected contents.
export function compareVendored({ provenance, files }, directory = VENDORED) {
  const problems = [];
  const present = fs.existsSync(directory) ? fs.readdirSync(directory).sort() : [];
  const expected = [...files.keys(), 'provenance.json'].sort();
  for (const name of present) if (!expected.includes(name)) problems.push(`unexpected ${name}`);
  for (const name of expected) if (!present.includes(name)) problems.push(`missing ${name}`);
  for (const [name, bytes] of files)
    if (present.includes(name) && !fs.readFileSync(path.join(directory, name)).equals(bytes)) problems.push(`${name} differs`);
  if (present.includes('provenance.json') && fs.readFileSync(path.join(directory, 'provenance.json'), 'utf8') !== provenanceText(provenance))
    problems.push('provenance.json differs');
  return problems;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const args = process.argv.slice(2);
    const unknown = args.filter(arg => arg !== '--check');
    if (unknown.length) throw new Error(`Usage: node tools/vendor-typescript-libs.mjs [--check]; unknown ${unknown.join(' ')}`);
    const expected = collect();
    if (args.includes('--check')) {
      const problems = compareVendored(expected);
      if (problems.length) throw new Error(`${path.relative(REPOSITORY, VENDORED)} needs regeneration: ${problems.join('; ')}`);
    } else {
      fs.rmSync(VENDORED, { recursive: true, force: true });
      fs.mkdirSync(VENDORED, { recursive: true });
      for (const [name, bytes] of expected.files) fs.writeFileSync(path.join(VENDORED, name), bytes);
      fs.writeFileSync(path.join(VENDORED, 'provenance.json'), provenanceText(expected.provenance));
    }
    console.log(`TypeScript ${VERSION} libraries ${args.includes('--check') ? 'verified' : 'vendored'}: ${expected.files.size - LICENSE_FILES.length} files`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
