# Implicit-any reporting

The operating envelope is the ordinary TypeScript query profile. The native
7.0.2 hook and public API agree on all 30 selected roots, captured in
`vrun-0c57e9d2-0438-42b0-8ac7-20b25cbd2203`. That initial capture failed only
because expected files were absent; these baselines contain its exact rows.

Anonymous annotations force the serializer to acquire parameter types. A
nominal type-alias summary does not, so the earlier alias-name capture is not
reporting evidence. The native diagnostics distinguish 7051 for type-keyword
and Type-meaning parameter names from 7006 for value-only/absent names. A
constructor type and ordinary function parameters use the latter branch;
ordinary rest parameters use 7019. Escaped `number` names retain the same
keyword classification. Disabling noImplicitAny preserves types and omits
these diagnostics; loose null checking widens nested null/undefined and empty
array properties with three 7018 reports.

`Poison` has an invalid type definition, but parameter-name reporting asks
only for its raw Type symbol. It must not evaluate that definition. The final
Project test demands summary and expansion in both request orders and expects
healthy results throughout; it does not treat diagnostic text as invalidity.
