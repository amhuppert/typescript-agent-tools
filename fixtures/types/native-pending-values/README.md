# Exact property names and consumed Object recovery

The original eight queries keep their captured source and no-library config.
The public compiler result is `number` even for the six call-contained names;
that result alone does not prove that their checking prelude is healthy.

The maintained hook observes only the actual `getPropertyOfType` branch that
queries `globalObjectType`. All six call-contained names reach that branch.
With `tsconfig.json`, its `emptyRecovery` observation is true; with `object.json`
it is false. The latter adds only an actual `interface Object {}` declaration.
The two direct outside-call queries never emit this observation. Startup
diagnostics for other missing globals remain in both profiles and do not by
themselves establish a consumed semantic dependency.

Measured with TypeScript 7.0.2, native run
`vrun-d08d2ce7-90d9-4375-ba4d-27f1f59ff75c`. Baselines retain the measured
pre-summary observations and diagnostic membership, not inferred demand order.
Unpaired names still fail the product's exact-name admission before its Object
lookup; their diagnostic spelling in the hook JSON is not a semantic identity.
