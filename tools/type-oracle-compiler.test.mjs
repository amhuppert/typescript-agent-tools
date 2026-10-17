import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {compiledOracle} from './type-oracle-compiler.mjs';

test('compiled code reuse ignores temporary paths and keeps source/toolchain changes distinct', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'type-oracle-cache-'));
  const previous = process.cwd();
  process.chdir(directory);
  try {
    const source = path.join(directory, 'input.go');
    fs.writeFileSync(source, 'package checker\n');
    const overlays = {'/pinned/internal/checker/overlay.go': source};
    let buildCount = 0;
    let environmentCount = 0;
    let arch = 'amd64';
    const compiler = (_binary, args) => {
      if (args[0] === 'env') return JSON.stringify({GOARCH: arch, GOVERSION: 'go1.26.0', CGO_CFLAGS: '-O2', GOGCCFLAGS: `-ffile-prefix-map=/tmp/go-build${++environmentCount}=/tmp/go-build`});
      assert.equal(args[0], 'test');
      const target = args[args.indexOf('-o')+1];
      fs.writeFileSync(target, `compiled code ${++buildCount}`+'x'.repeat(128 * 1024));
      return '';
    };
    const first = compiledOracle('/pinned', 'commit', overlays, compiler);
    const second = compiledOracle('/pinned', 'commit', overlays, compiler);
    assert.equal(second, first);
    assert.equal(buildCount, 1);
    fs.writeFileSync(source, 'package checker\n// changed\n');
    const changed = compiledOracle('/pinned', 'commit', overlays, compiler);
    assert.notEqual(changed, first);
    assert.equal(buildCount, 2);
    arch = 'arm64';
    const otherArch = compiledOracle('/pinned', 'commit', overlays, compiler);
    assert.notEqual(otherArch, changed);
    assert.equal(buildCount, 3);
    // Damage past two hash chunks, so validation must read the whole file.
    const damaged = fs.readFileSync(otherArch);
    damaged[damaged.length-1] ^= 1;
    fs.writeFileSync(otherArch, damaged);
    assert.throws(() => compiledOracle('/pinned', 'commit', overlays, compiler), /does not match its manifest/);
    assert.equal(buildCount, 3, 'corruption stays a failure');
  } finally {
    process.chdir(previous);
    fs.rmSync(directory, {recursive: true, force: true});
  }
});
