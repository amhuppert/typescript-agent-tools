# Historical representation experiment

These are exact copies of the reviewed experiment's code and raw measurements.
See [the report](../../../docs/types-representation-evidence.md) for scope,
results, hashes and remaining obligations. The independent review is available
in Git history. No production representation is
selected by this experiment.

The scripts preserve their original `.cc/temp/types-representation` working
layout and pinned local corpus paths. To reproduce, restore that layout and the
manifest at `.cc/temp/types-baseline/command-center-manifest.json`, using
`fixtures/types/command-center/input-manifest.json`. Capture requires the pinned
corpus revision and all recorded source hashes. The 6.8 MiB source captures and
compiled binaries are excluded from this archive; the scripts recreate the
captures and `build-commands.json` records the measured build arguments.

`provenance.json` binds the historical code, inputs, binaries and raw results.
Timing uses five uninstrumented warm-process rounds; requested heap uses two
separate instrumented rounds. These are prototype costs, not complete native
query, load, refresh or service measurements.
