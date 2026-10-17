# Checked Promise factory recovery

TypeScript 7.0.2 `elaborateArrowFunction` checks the incompatible return types,
elaborates the body, and creates a return diagnostic. For a synchronous arrow
whose return type has no `then`, it calls `createPromiseType` while considering
the related “mark this function as async” suggestion. That factory asks
`getGlobalPromiseTypeChecked` before any awaited-type work. An invalid global
returns the actual `unknownType`, while the declaration's final public summary
remains `() => number`.

The hook observes only the acquired global pointer immediately after that
checked getter, comparing it with `emptyGenericType`. It makes no additional
semantic call. The sixteen queries distinguish valid, absent, non-object alias,
and wrong-arity interface globals. The failed arrow assignment is the genuine
factory witness; structural `await` and the compatible arrow skip the factory.
The comma case performs quiet structural awaiting before checked construction
in the same checker, proving that quiet lookup cannot suppress checked-global
acquisition and its diagnostic.

Initial measured capture: `vrun-4b514bdc-1816-4217-bf18-0163f974224f`.
Its four groups failed only because expected files had not yet been installed.
Baselines are the measured pre-summary results. Diagnostic arrays describe
membership, not acquisition order. Final product tests retain consumed global
recovery despite the native healthy-looking summary, and compare exact source
locations separately from the optional global declaration cause.
