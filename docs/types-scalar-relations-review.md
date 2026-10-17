# Scalar relations and contextual applicability review

This bounded Ousterhout review assumes one accepted immutable Project and a
request-local evaluator. It reads `evaluation/relations.rs`, `contextual.rs`
and call applicability against pinned `relater.go::isTypeRelatedTo`,
`isSimpleTypeRelatedTo`, union comparison and the contextual-literal checker
operations. It finds no additional false successful answer in the admitted
intrinsic/literal and primitive-union slice.

Native fresh literal regularization and exact TypeId equality precede related
type work. Comparable tries the reverse simple relation only for a non-Never
target, then the forward simple relation. Genuine Any/Unknown/Never and strict
nullable early branches keep their native order. Boolean retains semantic Union
flags independently from its displayed primitive view. Unsupported structured
or instantiable dependencies are errors, rather than false relations later
treated as an ordinary overload rejection.

The intrinsic union operations preserve native source-each/source-some and
target-some direction. Contextual scalar literal preservation uses the actual
contextual category and regular literal identity; it is distinct from ordinary
expression widening and has its own question cache. Instantiable contexts ask
the complete base-constraint owner before genuine absent constraint becomes
Unknown. Failure is not converted to constraint absence. Primitive wrapper,
anonymous contextual object and broad contextual inference dependencies remain
explicit operations yet to be completed.

Call applicability consumes this/parameter/contextual argument questions in
their native order before the actual relation. Plain identifier invocation has
the native Void receiver, and a declared Void this parameter follows its
separate early branch. Spread/rest/tuple and non-retained receiver/contextual
families remain precise pending dependencies. Actual graph/property origins are
not used as type or signature identity.

Final argument five-test GREEN is
`vrun-5bb6ed70-2c39-4e3f-b3fb-96dc8fe5b35b`, after genuine Project RED
`vrun-3c2c3584`. The new deferred anonymous member increment moves complete
`IsEmptyAnonymousObjectType` into the evaluator: factory kind decides whether
actual inventory is needed, and failed inventory cannot become empty. That
later coupled increment still needs its own final proof.

This review does not prove structural relation normalization/cache/Maybe/limit
protocols, enum equality, intersections, variance, contextual objects, generic
inference or full overload reorder/retry. Two unreachable enum flag constants
were mechanically corrected in the sole Model flag owner: EnumLiteral is bit
15 and Unit includes Enum bit 16. No enum factory is admitted by that correction,
and no enum behavior RED/GREEN is claimed.
