# Native Array classification control

This is one fresh TypeScript 7.0.2 same-checker observation over the existing
`json-array-targets/valid-members` fixture. Its global scope declares `Array<T>`
and omits `ReadonlyArray<T>`. The imported JSON property has native summary
`number[]`, a non-error root, and these actual native facts:

| Question | Observed value |
| --- | --- |
| `isArrayType` | true |
| `isReadonlyArrayType` | true |
| `Target() == globalArrayType` | true |
| `Target() == globalReadonlyArrayType` | true |

Checker initialization makes missing `ReadonlyArray` fall back to the Array
target (`checker.go` 1365–1368). The internal readonly predicate therefore
returns true (`checker.go` 23393), while `typeReferenceToTypeNode` gives the
mutable Array target precedence and prints `number[]` (`nodebuilderimpl.go`
2949–2963). The public array display flag and the native readonly relation
predicate must remain separate facts.

`setup.mjs` and `probe.mjs` are archived scratch measurement code. They ran
from `.cc/temp/types-array-classification`; their original absolute imports
remain intact. The scratch checker hook adds only an observation method; it
does not perform a new type demand or change a native predicate. `answers.json`
preserves the original response, including exact captured replay input hashes,
checking prelude and phase observations. The native checker already records
unrelated missing-global diagnostics for this deliberately small noLib input;
the imported Array factory consumes the valid Array target. This control does
not assert whole-program diagnostic freedom.

`manifest.json` freezes the archived control files and input copies. Input
copies were compared with the replay hashes after the run. Tool-owner and
native-source hashes are post-run inspection evidence, not an atomic tool
installation snapshot. This README was added after the manifest and is not
part of that frozen file set. No registered product test, general readonly
normalization, performance, or full generic-target gate is claimed here.
