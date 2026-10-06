// Node test reporter for the harness validation wrappers. The first line holds
// the run's final counts, "TESTS EXECUTED PASSED SKIPPED TODO"; each further
// line names one skipped test or suite, "skipped: FILE: NAME (REASON)". Nothing
// is written when the runner reports no final summary. What the counts mean
// for the gate is decided by run_harness_tests in scripts/validate/common.sh.
import fs from 'node:fs';
import path from 'node:path';

export default async function* summaryReporter(source) {
  const root = fs.realpathSync(process.cwd());
  const skips = [];
  let counts;
  for await (const { type, data } of source) {
    if ((type === 'test:pass' || type === 'test:fail') && data.skip !== undefined) {
      const file = data.file ? path.relative(root, fs.existsSync(data.file) ? fs.realpathSync(data.file) : data.file) : '<unknown file>';
      skips.push(`skipped: ${file}: ${data.name}${typeof data.skip === 'string' ? ` (${data.skip})` : ''}\n`);
    } else if (type === 'test:summary' && data.file === undefined) {
      counts = data.counts;
    }
  }
  if (!counts) return;
  const { tests, passed, skipped, todo } = counts;
  yield `${tests} ${tests - skipped} ${passed} ${skipped} ${todo}\n`;
  yield* skips;
}
