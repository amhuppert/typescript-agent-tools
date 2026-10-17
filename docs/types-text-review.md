# Type text renderer review

This read-only Ousterhout review covers `src/cli/types.rs` as a consumer of one
already admitted type document. It does not prove future semantic families or
substitute for the final CLI tests owned by the renderer implementer.

No blocking issue was found for the current scalar and ordinary public object
family. The renderer never opens sources or calls the model. It walks document
nodes once, resolves edge labels by graph ID without following edges recursively,
and therefore renders shared/cyclic references finitely. JSON quoting preserves
exact keys and escapes controls. Complete and partial headings are separate from
display omissions, and ambiguous selection text uses the supplied replay anchor.

One pending full-contract issue is visible in `members_text`: property kind and
accessibility are not rendered. A protected accessor, private ordinary property
and public method can therefore lose their distinguishing facts in text despite
those facts surviving in JSON. Add explicit labels when the corresponding native
family is admitted, with a final CLI reproduction using the same structured
result. This is not a claimed currently reachable bug: the proved first family
contains only public property assignments. Alias labels likewise need a final
collision/qualification fixture when real alias metadata becomes available;
the current layout prints names but not their origins.

The linear graph layout and JSON quoting are justified boundaries, not redundant
semantic helpers. Pulling traversal or identifier classification into this
renderer would duplicate decisions already owned by the library.
