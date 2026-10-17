# Cold Void defers its operand

Pinned TypeScript 7.0.2 checker.go10814 calls checkNodeDeferred and returns the
undefined-widening intrinsic. Deferred operand checking occurs when source-file
checking drains checkDeferredNodes; a cold type query does not drain that queue.

The four roots run as separate cold checker-hook and compiler-API sessions under
strict, loose and noCheck. `(void first, 65)` returns 65 without acquiring the
missing first annotation; `(void first, later)` acquires only MissingVoidLater;
`void first` does not acquire the annotation and its variable type is Undefined
under strict/noCheck and healthy Any under loose widening. `(first, 65)` supplies
the positive instrumentation control: it acquires MissingVoidFirst and still
returns the healthy literal 65. The hook leaf observations are exact
getTypeFromTypeNode error exits, while final native/API errorType parity is
checked independently; propagated error exits are not treated as new leaf
acquisitions.

Registered proof: vrun-92ef24e8-9d32-4f04-a6f6-7712d8729f71, three tests,
twelve cold native/API pairs, zero skips. Initial captures corrected two probe
assumptions: native preserves the unresolved alias spelling in the final
summary, and loose variable widening converts Undefined to healthy Any.

This is a counterexample to using Void operands as evidence that a cold query
executes expensive union, object or non-null work. Resource fixtures must use
actual eager operands and record native diagnostic-only outcomes rather than
changing Void checking to eager evaluation. No production Void behavior was
changed for this proof.
