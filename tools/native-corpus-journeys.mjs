// Repeatable product diagnostics against the completed pinned native capture.
// The external corpus is required; absence is a failure, never a silent skip.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {assertMemberJourney} from './native-corpus-comparison.mjs';
import {verifyCapturedInputs} from './type-corpus-capture.mjs';
import {measuredRun, sampleResidentBytes} from './native-corpus-resources.mjs';

const root = process.cwd();
const corpus = path.join(root, '.cc/temp/command-center-aa484f4-corpus');
const evidence = path.join(root, 'fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native');
const binary = path.resolve(process.env.TSPARSE ?? 'target/release/tsparse');
const fullSummaryBytes = fs.readFileSync(path.join(evidence, 'no-truncation-observations.json'));
const fullSummaryCapture = JSON.parse(fullSummaryBytes);
assert.equal(fullSummaryCapture.captureComplete, true);
assert.equal(fullSummaryCapture.observations.length, 1);
const fullSummary = fullSummaryCapture.observations[0];
assert.equal(fullSummary.id, 'predicate-callback');
assert.equal(fullSummary.request.noTruncation, true);
const service = process.argv.includes('--service');
const resources = process.argv.includes('--resources');
const rounds = resources ? 2 : 1;
const reportPath = path.join(root, service ? '.cc/temp/native-corpus-service-current.json' : '.cc/temp/native-corpus-current.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const exact = (left, right) => left < right ? -1 : left > right ? 1 : 0;
const revision = spawnSync('git', ['rev-parse', 'HEAD'], {cwd: corpus, encoding: 'utf8'});
assert.equal(revision.status, 0, revision.stderr);
assert.equal(revision.stdout.trim(), 'aa484f422f00dfb64967d2008fac3c4f9a92538e');
const tracked = spawnSync('git', ['diff', '--quiet', 'HEAD', '--'], {cwd: corpus});
assert.equal(tracked.status, 0, 'frozen corpus has tracked modifications');
assert.ok(fs.existsSync(path.join(corpus, 'tsconfig.native-corpus.json')));
const sourceList = spawnSync('rg', ['--files', '--hidden', '--no-ignore', 'src', 'vendor', '.cargo'], {cwd: root, encoding: 'utf8'});
assert.equal(sourceList.status, 0);
const sourceHashes = Object.fromEntries([...sourceList.stdout.trim().split('\n'), 'Cargo.toml', 'Cargo.lock', 'build.rs'].sort(exact).map(file => [file, hash(fs.readFileSync(path.join(root, file)))]));
const buildInputsPath = path.join(root, '.cc/temp/native-corpus-build-inputs.json');
if (process.argv.includes('--capture-build-inputs')) {
  const rustc = spawnSync(process.env.RUSTC ?? 'rustc', ['--version', '--verbose'], {encoding: 'utf8'});
  assert.ifError(rustc.error);
  assert.equal(rustc.status, 0, rustc.stderr);
  fs.writeFileSync(buildInputsPath, JSON.stringify({sourceHashes, rustc: rustc.stdout,
    rustflags: process.env.RUSTFLAGS ?? null, encodedRustflags: process.env.CARGO_ENCODED_RUSTFLAGS ?? null,
    node: process.version, platform: process.platform, arch: process.arch}, null, 2)+'\n');
  process.exit(0);
}
const buildInputs = JSON.parse(fs.readFileSync(buildInputsPath, 'utf8'));
assert.deepEqual(sourceHashes, buildInputs.sourceHashes, 'build inputs changed after the pre-build snapshot');
const report = {mode: service ? 'service' : 'direct', rounds, resourceMeasurements: resources ? {platform: process.platform, osRelease: os.release(), totalMemoryBytes: os.totalmem(), logicalProcessors: os.cpus().length, units: 'bytes', maximumResidentScope: 'timed CLI process', serviceResidentScope: 'sample after each query, not peak RSS'} : null, scope: process.argv.includes('--failed-only') ? 'failed-only' : 'full', replayComplete: false, buildInputs, sourceHashes, toolingHashes: Object.fromEntries(['tools/native-corpus-journeys.mjs', 'tools/native-corpus-resources.mjs', 'tools/native-corpus-comparison.mjs', 'tools/type-corpus-capture.mjs', 'scripts/validate/native-corpus-journeys-full.sh', 'scripts/validate/common.sh'].map(file => [file, hash(fs.readFileSync(path.join(root, file)))])), corpusRevision: revision.stdout.trim(), binarySha256: hash(fs.readFileSync(binary)), profile: 'tsconfig.native-corpus.json', journeys: []};
fs.writeFileSync(reportPath, JSON.stringify(report, null, 2)+'\n');
let direct;
if (service) {
  direct = JSON.parse(fs.readFileSync(path.join(root, '.cc/temp/native-corpus-current.json'), 'utf8'));
  assert.equal(direct.replayComplete, true, 'direct replay did not finish');
  assert.equal(direct.scope, report.scope);
  assert.equal(direct.rounds, rounds);
  assert.equal(direct.binarySha256, report.binarySha256);
  assert.deepEqual(direct.sourceHashes, report.sourceHashes);
}
// Unix socket names need a short private directory. Always stop the service
// before removing its state, including after a failed query or verification.
const runtime = service ? fs.mkdtempSync('/tmp/tsparse-corpus-') : undefined;
const env = service ? {...process.env, TSPARSE_SERVICE_DIR: runtime, TSPARSE_SERVICE_IDLE_SECS: '120'} : process.env;
function readServiceStatus() {
  const status = spawnSync(binary, ['--project', report.profile, '--references-status'], {cwd: corpus, env, encoding: 'utf8', timeout: 30_000});
  assert.ifError(status.error);
  assert.equal(status.status, 0, status.stderr);
  const value = JSON.parse(status.stdout);
  report.lastServiceStatus = value;
  assert.equal(value.running, true);
  assert.ok(Number.isInteger(value.pid) && value.pid > 0);
  assert.equal(value.generation, 1);
  if (report.serviceStatus) assert.deepEqual(value, report.serviceStatus, 'service restarted or refreshed');
  else report.serviceStatus = value;
  return value;
}
let failed = 0;
try {
  for (let round = 1; round <= rounds; round++) {
    for (const name of ['member-observations.json', 'type-observations.json']) {
      const bytes = fs.readFileSync(path.join(evidence, name));
      const captured = JSON.parse(bytes);
      assert.equal(captured.captureComplete, true);
      const inputs = new Map(captured.inputHashes.map(({file, sha256}) => [file, sha256]));
      assert.equal(inputs.size, captured.inputHashes.length);
      for (const row of captured.observations) for (const input of row.answer.inputHashes)
        assert.equal(inputs.get(input.file), input.sha256, input.file);
      verifyCapturedInputs(inputs);
      for (const row of captured.observations) {
        if (process.argv.includes('--failed-only') && !((name === 'member-observations.json') || row.id === 'predicate-callback')) continue;
        const member = row.request.operation === 'member';
        const {file, line, column} = row.request.at;
        const args = [...(service ? [] : ['--direct']), '--project', report.profile, member ? '--references' : '--type', '--json', '--at', `${line}:${column}`, file];
        const loadBefore = resources ? os.loadavg() : undefined;
        const started = performance.now();
        const run = (resources ? measuredRun : spawnSync)(binary, args, {cwd: corpus, env, encoding: 'utf8', timeout: 180_000, maxBuffer: 16 * 1024 * 1024});
        const observed = {id: row.id, round, ...(resources ? {loadBefore, loadAfter: os.loadavg(), maximumResidentBytes: run.maximumResidentBytes, resourceMeasurementFailure: run.resourceMeasurementFailure, resourceCleanupFailure: run.resourceCleanupFailure, resourceMonitorOutput: run.resourceMonitorOutput} : {}), operation: row.request.operation, nativeCaptureSha256: hash(bytes), args, elapsedMilliseconds: performance.now() - started, exitCode: run.status, signal: run.signal, stdout: run.stdout, stderr: run.stderr};
        try {
          if (run.stdout.trim()) observed.result = JSON.parse(run.stdout);
          assert.ifError(run.error);
          if (resources) {
            assert.equal(run.resourceMeasurementFailure, undefined);
            assert.equal(run.resourceCleanupFailure, undefined);
            assert.ok(Number.isSafeInteger(run.maximumResidentBytes) && run.maximumResidentBytes > 0);
          }
          const answer = observed.result;
          if (member) {
            assertMemberJourney(answer, row.answer, run.status);
          } else {
            assert.equal(run.status, 0, run.stderr);
            assert.equal(row.answer.errorType, false);
            assert.equal(answer.outcome, 'found');
            let summary = row.answer.summary;
            if (row.id === fullSummary.id) {
              assert.deepEqual(fullSummary.request.at, row.request.at);
              assert.deepEqual(fullSummary.answer.programFiles, row.answer.programFiles);
              assert.deepEqual(fullSummary.answer.inputHashes, row.answer.inputHashes);
              assert.deepEqual(fullSummary.answer.compilerOptions, row.answer.compilerOptions);
              assert.equal(fullSummary.answer.errorType, false);
              summary = fullSummary.answer.summary;
              observed.fullSummaryCaptureSha256 = hash(fullSummaryBytes);
            }
            assert.equal(answer.type.summary, summary, row.id);
            assert.deepEqual(answer.issues, []);
          }
          if (service) {
            const paired = direct.journeys.filter(item => item.id === row.id && item.operation === row.request.operation && item.round === round);
            assert.equal(paired.length, 1);
            assert.deepEqual([run.status, run.stdout, run.stderr], [paired[0].exitCode, paired[0].stdout, paired[0].stderr]);
            observed.serviceStatus = readServiceStatus();
            if (resources) observed.serviceResidentSampleBytes = sampleResidentBytes(observed.serviceStatus.pid);
          }
          observed.passed = true;
        } catch (error) {
          failed++;
          observed.passed = false;
          observed.failure = String(error);
        }
        report.journeys.push(observed);
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2)+'\n');
        console.log(`${member ? 'Q3' : 'Q4'} ${row.id} round ${round}: ${observed.passed ? 'passed' : 'failed'} (${Math.round(observed.elapsedMilliseconds)} ms)`);
      }
      verifyCapturedInputs(inputs);
    }
  }
  assert.equal(report.journeys.length, rounds * (report.scope === 'full' ? 8 : 4));
  for (const [file, expected] of Object.entries({...report.sourceHashes, ...report.toolingHashes}))
    assert.equal(hash(fs.readFileSync(path.join(root, file))), expected, `source changed during replay: ${file}`);
  assert.equal(hash(fs.readFileSync(binary)), report.binarySha256, 'binary changed during replay');
  if (service) readServiceStatus();
  report.replayComplete = true;
} catch (error) {
  failed++;
  report.replayFailure = String(error);
} finally {
  if (service) {
    const stopStarted = performance.now();
    const stopped = spawnSync(binary, ['--project', report.profile, '--references-stop'], {cwd: corpus, env, encoding: 'utf8', timeout: 30_000});
    report.serviceStop = {exitCode: stopped.status, stdout: stopped.stdout, stderr: stopped.stderr, error: stopped.error && String(stopped.error)};
    if (stopped.error || stopped.status !== 0) {
      failed++;
      report.replayComplete = false;
      report.serviceRuntime = runtime;
    } else {
      const pid = report.lastServiceStatus?.pid;
      let exited = false;
      while (pid && !exited && performance.now() - stopStarted < 30_000) {
        try {
          process.kill(pid, 0);
        } catch (error) {
          if (error.code !== 'ESRCH') throw error;
          exited = true;
        }
        if (!exited) await new Promise(resolve => setTimeout(resolve, 25));
      }
      report.serviceStop.processExited = pid ? exited : null;
      report.serviceStop.elapsedMilliseconds = performance.now() - stopStarted;
      if (exited) fs.rmSync(runtime, {recursive: true, force: true});
      else {
        failed++;
        report.replayComplete = false;
        report.serviceRuntime = runtime;
      }
    }
  }
  report.passed = report.replayComplete && failed === 0;
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2)+'\n');
}
console.log(`${report.mode}: ${report.journeys.filter(row => row.passed).length}/${report.journeys.length} corpus journeys passed; ${reportPath}`);
if (failed) process.exitCode = 1;
