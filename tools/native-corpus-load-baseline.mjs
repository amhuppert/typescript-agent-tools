// Measure a lexical request, including normal checker initialization but no member/type demand.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {measuredRun, sampleResidentBytes} from './native-corpus-resources.mjs';
import {verifyCapturedInputs} from './type-corpus-capture.mjs';

const root = process.cwd();
const corpus = path.join(root, '.cc/temp/command-center-aa484f4-corpus');
const binary = path.resolve(process.env.TSPARSE ?? 'target/release/tsparse');
const hash = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const prior = JSON.parse(fs.readFileSync('.cc/temp/native-corpus-current.json', 'utf8'));
assert.equal(prior.replayComplete, true);
assert.equal(prior.mode, 'direct');
assert.equal(prior.scope, 'full');
assert.equal(prior.journeys.length, 16);
assert.ok(prior.journeys.every(row => row.passed));
assert.equal(hash(binary), prior.binarySha256);
const evidence = 'fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native';
const inputs = new Map();
for (const name of ['member-observations.json', 'type-observations.json']) {
  const captured = JSON.parse(fs.readFileSync(path.join(evidence, name), 'utf8'));
  assert.equal(captured.captureComplete, true);
  for (const input of captured.inputHashes) {
    if (inputs.has(input.file)) assert.equal(inputs.get(input.file), input.sha256);
    inputs.set(input.file, input.sha256);
  }
}
verifyCapturedInputs(inputs);
for (const [file, expected] of Object.entries(prior.sourceHashes)) assert.equal(hash(file), expected, file);
const report = {kind: 'registered_corpus_load_baseline', complete: false,
  binarySha256: prior.binarySha256, sourceHashes: prior.sourceHashes,
  toolingHashes: Object.fromEntries(['tools/native-corpus-load-baseline.mjs', 'tools/native-corpus-resources.mjs',
    'scripts/validate/native-corpus-load-baseline-full.sh'].map(file => [file, hash(file)])),
  rounds: 2, journeys: [], scope: 'lexical name query including checker initialization; no member/type demand',
  units: 'bytes', serviceResidentScope: 'sample after query, not peak RSS'};
const output = '.cc/temp/native-corpus-load-current.json';
const save = () => fs.writeFileSync(output, JSON.stringify(report, null, 2)+'\n');
save();
let failed = 0;
for (const service of [false, true]) {
  const runtime = service ? fs.mkdtempSync('/tmp/tsparse-load-') : undefined;
  const env = service ? {...process.env, TSPARSE_SERVICE_DIR: runtime, TSPARSE_SERVICE_IDLE_SECS: '120'} : process.env;
  let pid;
  if (service) { report.serviceRuntime = runtime; save(); }
  try {
    for (let round = 1; round <= 2; round++) {
      const args = [...(service ? [] : ['--direct']), '--project', prior.profile,
        '--references', '--json', '--name', '__tsparse_resource_calibration_absent__'];
      const loadBefore = os.loadavg();
      const started = performance.now();
      const run = measuredRun(binary, args, {cwd: corpus, env, timeout: 180_000, maxBuffer: 1024 * 1024});
      const row = {mode: service ? 'service' : 'direct', round, args, loadBefore, loadAfter: os.loadavg(),
        elapsedMilliseconds: performance.now() - started, exitCode: run.status,
        stdout: run.stdout, stderr: run.stderr, maximumResidentBytes: run.maximumResidentBytes,
        resourceMeasurementFailure: run.resourceMeasurementFailure,
        resourceCleanupFailure: run.resourceCleanupFailure, resourceMonitorOutput: run.resourceMonitorOutput};
      report.journeys.push(row);
      save();
      assert.ifError(run.error);
      assert.equal(run.resourceMeasurementFailure, undefined);
      assert.equal(run.resourceCleanupFailure, undefined);
      assert.equal(run.status, 0, run.stderr);
      const answer = JSON.parse(run.stdout);
      assert.equal(answer.outcome, 'complete');
      assert.equal(answer.coverage.baseline, 'lexical');
      assert.equal(answer.coverage.status, 'complete');
      assert.deepEqual(answer.coverage.gaps, []);
      assert.deepEqual(answer.groups, []);
      assert.deepEqual([run.status, run.stdout, run.stderr],
        [report.journeys[0].exitCode, report.journeys[0].stdout, report.journeys[0].stderr]);
      if (service) {
        const status = spawnSync(binary, ['--project', prior.profile, '--references-status', '--json'],
          {cwd: corpus, env, encoding: 'utf8', timeout: 30_000});
        assert.ifError(status.error);
        assert.equal(status.status, 0, status.stderr);
        row.serviceStatus = JSON.parse(status.stdout);
        assert.equal(row.serviceStatus.running, true);
        assert.equal(row.serviceStatus.generation, 1);
        if (pid) assert.equal(row.serviceStatus.pid, pid);
        pid = row.serviceStatus.pid;
        row.serviceResidentSampleBytes = sampleResidentBytes(pid);
      }
      row.passed = true;
      save();
    }
  } catch (error) {
    failed++;
    report.failure = String(error);
    save();
  } finally {
    if (service) {
      const started = performance.now();
      const stop = spawnSync(binary, ['--project', prior.profile, '--references-stop'],
        {cwd: corpus, env, encoding: 'utf8', timeout: 30_000});
      report.serviceStop = {exitCode: stop.status, stdout: stop.stdout, stderr: stop.stderr,
        error: stop.error && String(stop.error)};
      save();
      assert.ifError(stop.error);
      assert.equal(stop.status, 0, stop.stderr);
      while (pid) {
        try { process.kill(pid, 0); }
        catch (error) { if (error.code !== 'ESRCH') throw error; break; }
        assert.ok(performance.now() - started < 30_000, 'baseline service did not exit');
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      report.serviceStop.processExited = pid ? true : null;
      report.serviceStop.elapsedMilliseconds = performance.now() - started;
      fs.rmSync(runtime, {recursive: true});
      delete report.serviceRuntime;
      save();
    }
  }
}
verifyCapturedInputs(inputs);
for (const [file, expected] of Object.entries({...report.sourceHashes, ...report.toolingHashes}))
  assert.equal(hash(file), expected, file);
assert.equal(hash(binary), report.binarySha256);
report.complete = failed === 0 && report.journeys.length === 4;
save();
if (!report.complete) throw new Error(`load baseline failed: ${report.failure}`);
