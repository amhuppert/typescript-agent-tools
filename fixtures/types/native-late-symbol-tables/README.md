This fixture pins TypeScript 7.0.2 `getResolvedMembersOrExportsOfSymbol`,
`lateBindMember`, `lateBindIndexSignature`, `getLateBoundSymbol`, and their
declaration and index consumers. Each marker is a final Q4 type query; the
native harness compares the checker hook with the ordinary pinned API.

The main strict/loose profiles contain 42 rows each. They cover early and
computed string/number/unique-symbol names, static/instance isolation,
eligible entity syntax, duplicate/accessor/overload merging, broad computed
indexes, explicit-index precedence, recursion and demanded recovery. Healthy
final native output does not erase consumed erroneous names or annotations.
The Rust test requires those dependencies separately.

Additional independent profiles cover nine owner rows, seven metadata rows
and four combined-meaning rows per strictness setting, for 124 observations
in total. The owner controls preserve a native asymmetry: a late setter
finds its paired getter after late binding, but an early setter does not
force a late getter and remains Any. Metadata controls expose readonly index
aggregation and optional property flags in the final anonymous summary.
Combined-meaning controls retain both a computed static property's value
and the same-named namespace TypeAlias/Interface meaning.

Evidence:

- Main capture: `vrun-6b87df1f-20e2-4cf2-8c97-539e4becb797`.
- Main maintained comparison: `vrun-cbef673f-a8db-4f70-bc8b-de7d635b5fba`.
- Main Rust RED: `vrun-617ba02e-7e72-4dda-b3d3-1d57310c9bca`; six groups
  reach the intended unimplemented late-table guard after successful load.
- Owner capture: `vrun-e661ce5d-027e-4867-bda5-ba97c09a8a91`.
- Owner maintained comparison: `vrun-00dfe08c-361e-4f46-92b8-8102cbaa2baa`.
- Metadata capture: `vrun-3dd39cc7-eccf-4840-92ac-fa60bcf4c493`.
- Metadata maintained comparison and combined-meaning capture:
  `vrun-55e32618-a35e-4de7-ad19-6eca552b0657`.
- Combined-meaning maintained comparison:
  `vrun-fc3df1f0-7831-4e0e-bd03-7f63aaba157f`.

The harnesses require saved baselines on maintained runs. Native diagnostic
codes alone do not establish a consumed ErrorType; acquisition traces and
the native operation sequence establish the Rust recovery expectations.
Rust completion of the slice remains pending; these native receipts do not
claim a Rust GREEN or full-project validation.
