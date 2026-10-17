// Compile immutable oracle instrumentation once; every replay still starts a
// fresh process/checker and receives its own captured filesystem and input.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';

const digest = value => createHash('sha256').update(value).digest('hex');

// Executables are much larger than overlays. Verify every byte without
// allocating an executable-sized Buffer on each fresh query.
function fileDigest(file) {
  const descriptor = fs.openSync(file, 'r');
  try {
    const hash = createHash('sha256');
    const buffer = Buffer.allocUnsafe(64 * 1024);
    for (;;) {
      const length = fs.readSync(descriptor, buffer, 0, buffer.length, null);
      if (length === 0) return hash.digest('hex');
      hash.update(buffer.subarray(0, length));
    }
  } finally {
    fs.closeSync(descriptor);
  }
}

function installed(directory, identity) {
  try {
    fs.statSync(directory);
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') return null;
    throw error;
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'manifest.json'), 'utf8'));
  const binary = path.join(directory, 'oracle.test');
  if (manifest.identity !== identity || fileDigest(binary) !== manifest.binarySha256)
    throw new Error('Compiler hook cached executable does not match its manifest');
  return binary;
}

/** Cache only compiled code, keyed by the pinned source, complete Go build
 * environment and exact overlay contents/destinations. Filesystem-completion
 * retries and different requests never reuse a checker or observation stream.
 */
export function compiledOracle(sourceDirectory, sourceCommit, replace, runProcess) {
  const environment = {...process.env, GOMAXPROCS: '2'};
  const goEnvironment = JSON.parse(runProcess('go', ['env', '-json'], {
    cwd: sourceDirectory, env: environment,
  }));
  // This derived field embeds a fresh go-build temporary directory on each
  // `go env` invocation. Its inputs (CC, CGO_* flags, platform and toolchain)
  // remain in the key; the generated debug-prefix mapping is not a build input.
  delete goEnvironment.GOGCCFLAGS;
  const overlays = Object.entries(replace).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .map(([destination, source]) => ({destination, contents: fs.readFileSync(source)}));
  const description = {
    schema: 1, sourceDirectory, sourceCommit, goEnvironment,
    overlays: overlays.map(({destination, contents}) => ({destination, sha256: digest(contents)})),
    command: ['go', 'test', '-c', '-p', '2', './internal/ls'],
  };
  const identity = digest(JSON.stringify(description));
  const root = path.resolve('.cache/type-oracle-compilers');
  const directory = path.join(root, identity);
  const cached = installed(directory, identity);
  if (cached) return cached;
  fs.mkdirSync(root, {recursive: true});
  const staging = fs.mkdtempSync(path.join(root, `${identity}.build-`));
  try {
    const replacements = Object.fromEntries(overlays.map(({destination, contents}, index) => {
      const source = path.join(staging, `overlay-${index}.go`);
      fs.writeFileSync(source, contents);
      return [destination, source];
    }));
    const overlay = path.join(staging, 'overlay.json');
    fs.writeFileSync(overlay, JSON.stringify({Replace: replacements}));
    const binary = path.join(staging, 'oracle.test');
    runProcess('go', ['test', '-c', '-p', '2', '-overlay', overlay, '-o', binary, './internal/ls'], {
      cwd: sourceDirectory, env: environment,
    });
    fs.writeFileSync(path.join(staging, 'manifest.json'), JSON.stringify({
      ...description, identity, binarySha256: fileDigest(binary),
    }, null, 2)+'\n');
    // Two workers may build the same missing entry. Publish a whole directory;
    // the losing worker validates the winner instead of exposing partial code.
    try {
      fs.renameSync(staging, directory);
    } catch (error) {
      if (error.code !== 'EEXIST' && error.code !== 'ENOTEMPTY') throw error;
    }
    const published = installed(directory, identity);
    if (!published) throw new Error('Compiler hook cache publication is absent');
    return published;
  } finally {
    fs.rmSync(staging, {recursive: true, force: true});
  }
}
