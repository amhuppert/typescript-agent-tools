import assert from 'node:assert/strict';
const exact = (left, right) => left < right ? -1 : left > right ? 1 : 0;
const ordered = rows => [...rows].sort((a, b) => exact(JSON.stringify(a), JSON.stringify(b)));
export function assertMemberJourney(answer, native, exitCode) {
  assert.notEqual(answer?.outcome, 'error', `member query failed: ${answer?.error?.kind} (${answer?.error?.reason})`);
  assert.ok(answer?.coverage && Array.isArray(answer.coverage.gaps), 'member query lacks a coverage ledger');
  assert.equal(exitCode, answer.coverage.gaps.length ? 1 : 0);
  const normalized = ordered(answer.groups.map(group => ({declarations:ordered(group.declarations.map(item=>item.location)), occurrences:ordered(group.occurrences.map(item=>item.location))})));
  const gaps = answer.coverage.gaps;
  const candidates = new Set(native.candidateLedger.map(candidate => JSON.stringify(candidate.location)));
  assert.ok(gaps.every(gap => candidates.has(JSON.stringify(gap.location)) && !gap.omittedCauses), 'unobserved candidate or omitted cause');
  const excluded = reason => ['flow_required', 'jsx_typing_required'].includes(reason);
  assert.ok(gaps.every(gap => excluded(gap.reason) && (gap.causes ?? []).every(cause => excluded(cause.reason))), 'unexpected semantic coverage gap');
  assert.equal(answer.outcome,gaps.length?'partial':'complete');
  const omitted = new Set(gaps.map(gap=>JSON.stringify(gap.location)));
  const expected = ordered(native.nativeRangeGroups.map(group=>({declarations:ordered(group.origins.map(item=>item.location)),occurrences:ordered(group.occurrences.filter(location=>!omitted.has(JSON.stringify(location))))})).filter(group=>group.occurrences.length));
  assert.deepEqual(normalized,expected);
}
