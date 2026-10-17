# Native type facts operation

The operating envelope is one request-local Evaluator with established native
type identities and deferred constraint/member questions. Facts serve unary
checking and identity-based read admission; presentation never supplies flags,
shape guesses or source context to this operation.

Two designs were considered. A pure Model facts method taking callbacks would
make callers supply base-constraint, member and subtype policy and split the
native schedule across layers. One evaluator child owning getTypeFacts,
getTypeWithFacts and getAdjustedTypeWithFacts keeps those decisions together.
The latter is selected. Model retains the sole established TypeFlags owner and
filterType identity construction. It exposes semantic union constituents even
when the public view prints Boolean as primitive.

The facts worker retains native flag branch order, including Boolean before
Union, and passes the caller's actual mask to every recursive question. Object
facts first apply the mask early-out; only a relevant mask resolves the actual
whole inventory. Empty Anonymous shape and isFunctionObjectType are separate
questions. Function objects use actual call/construct signatures, then the
native bind/global-Function subtype branch. Interface/Class or nominal display
metadata never substitutes for that operation. Constraint acquisition remains
the existing specialized lazy evaluator question, with its original failure.

filterType preserves the original identity when every constituent survives,
returns the ordinary Never identity when none survive, and leaves any native
Never sentinel unchanged. Reduced unions retain canonical semantic order;
origins and aliases require their own complete owner as richer union families
arrive. getAdjustedTypeWithFacts preserves native unknown-union initialization
and recombination before nullable adjustment. NonNullable alias/intersection
dependencies must be established before claiming an adjusted healthy answer.

Fifty-eight fresh same-checker scratch sessions record the native full masks,
filter/adjusted summaries, flags and exact identity equality for 29 existing
primitive/literal/object/union families crossed with strictNullChecks. The
operation sequence is recorded, so post-demand facts observations are not
relabeled as an untouched baseline. Twelve fresh public unary controls and the
registered compiler gate `vrun-fbd00e10-b954-4ce6-9f79-9f47149fac09`
pin the first final Project behavior. Native data and exact generators are
preserved separately from production implementation claims.

The first final Project tests cover literal and primitive union unary masks,
strict/loose truthy unions, nullable unions and Never. Actual
`vrun-8a796481-5cfd-4437-80f3-2e2ae2a78ee2` compiled both tests and
failed both final assertions at the old composite-truthiness Pending branch.
Both now pass with zero skips in
`vrun-717427dc-632c-41d0-a808-44f4314644e6`. The earlier This DTO
visibility compile failure was setup evidence only. Root owns the next read-admission TDD: equal native
identity may be admitted, while changed identity remains the design's Flow
obligation. Equal identity from the facts helper is insufficient if the whole
narrowTypeByTypeof operation has further subtype/intersection/substitution
decisions. No nonunion shortcut or truthiness-only second facts map remains.

The complete native dependency branches, generic/free-constraint recursion,
intersections, global Function/NonNullable relationships, mapper/tuple facts,
native failure causality and resource/corpus gates remain required work. A
precise pending branch is an implementation boundary, not a scope reduction.
