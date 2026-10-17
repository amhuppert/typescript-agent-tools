# Circular resolution review

This review uses the Ousterhout reviewing route. The operating envelope is one
request-local native evaluator over an immutable captured project. The review
is limited to the admitted alias and variable-annotation questions, rather than
the future complete checker.

No blocking defect was found in `evaluation/resolution.rs`, the alias branch of
`declared_type()`, and their current parent dispatch. Pinned TypeScript 7.0.2
`pushTypeResolution`, `findResolutionCycleStartIndex`,
`typeResolutionHasProperty`, and `popTypeResolution` in
`internal/checker/checker.go` around 18663–18716 match the native stack's
canonical target/property keys, reverse search, barrier-before-key order,
participant marking, and rejected-push/no-pop rule. The alias operation around
23745 owns recovery at each marked declaration; an unmarked outer consumer
preserves the child's original failure. This is a useful deep operation:
callers do not infer circularity from a cache marker or output graph cycle.

The compiler allocates class/interface declared types separately, before their
members are resolved. The current admitted interface recipes likewise defer
member reads and do not use the alias resolution frame. Existing legal
recursive-interface and recursive-alias controls therefore exercise an
established canonical root, rather than treating `Resolving` as universal
invalidity. Broader early-construction re-entry remains explicitly pending.

The final circular tests reached assertion-level RED in
`vrun-503028c0-8bf6-42a3-8733-12627591e3b2` and passed all four tests with zero
skips in `vrun-89c6f1c8-b905-47e6-b54b-62892c06f091`. The controls include strict
and loose profiles, selected self/mutual participants, unmarked outer and
variable consumers, and healthy deferred recursion at depths 0, 1, 3, and 32.
This reviewer read the final source and terminal test disposition; no wider
resolution-property or checking-window proof is claimed.

The review prompted two behavior-preserving corrections before more operations
are admitted:

- `getTypeOfFuncClassEnumModuleWorker` around 16825 does not push a `Type`
  frame. It constructs an anonymous type, whereas
  `getTypeOfVariableOrParameterOrPropertyWorker` around 16491 owns that frame
  and `reportCircularityError`. The current shared value wrapper is inert for
  the first deferred function family, whose child demands occur after the
  wrapper returns. Root split the frame policy by actual family: only ordinary
  variables now push the Type frame. Functions keep their separate anonymous
  construction protocol and early re-entry remains explicitly pending.
- A failure kind alone cannot prove installation of a native recovery type.
  Root replaced `Failed(InvalidSemantic)` barrier detection with an explicit
  `Recovered` state, set only by the alias owner's actual marked pop/recovery
  path. Ordinary cached product failures do not establish a barrier. Additional
  native installation paths must deliberately extend that distinction when
  adding generic constraints, bases, returns, or other properties.

Post-correction circular and lazy compatibility checks passed with zero skips
in `vrun-86bb8c96-c178-45b7-ac5a-d128d3f21073` and
`vrun-ce939ebe-70e1-4453-9d84-9afe02230081` respectively. These checks preserve the existing admitted
behavior; they do not exercise future reentrant function/class operations.

`getResolvedSignature`'s temporary resolution window and sentinel, other
native entity/property targets, relation re-entry, and value circular recovery
remain pending. The source pin is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.
