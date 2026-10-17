# Native parameter selection and display spelling

This incremental public `Project::type_of` behavior uses the existing canonical
declared-parameter owner. It preserves the complete T/Q3/Q4 scope and does not
close generic-reference, token-inventory, resource or delivery gates.
TypeScript 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`
owns expected behavior.

The pinned fresh controls distinguish a TypeParameter's own declaration-name
question (`declared`) from its TypeReference use (`type`). Unrelated same-named
parameters have different canonical symbols and types; merged own parameter
declarations share one identity. Neither selecting an own parameter nor printing
its name demands an unused invalid constraint/default.

The escaped-name control exposed a real display distinction. A raw Identifier
`\u0056` has the exact decoded native symbol name `V`, but typeToString prints
`\u0056`. `getNameOfSymbolAsWritten` / `DeclarationNameToString` reads the first
named own declaration's spelling. Thus escaped-first/plain-later merged
declarations print the escape even when selecting the plain declaration, and
plain-first/escaped-later declarations print the plain name even when selecting
the escaped declaration. Public graph.name keeps the decoded identity; summary
keeps the native display spelling; target locations retain the selected raw
token's complete width.

Source's existing declaration-role owner now recognizes TypeParameter, and
selection routes it through the same declared-symbol operation. The shared
parameter recipe retains display_name separately from its decoded name, and the
presenter uses each for its own question. This avoids converting raw source text
into a new identity or reconstructing the canonical first declaration from
sorted display origins.

Registered compiler two-test GREEN:
`vrun-7575c35f-f946-4ea3-8fd5-c1b5486a2021`.
Registered final Project three-test RED:
`vrun-fa822dd3-c004-4f1f-97ed-5f4d70e569c0`.
It compiled, failing the missing own declaration selection and the existing
escaped type-use's incorrect `V` summary. The corresponding three-test GREEN is
`vrun-6d7bd031-64c6-4fbe-ae11-a98b88e786cc`.

The first-declaration ordering coverage then passed without another production
change: compiler two-test GREEN
`vrun-4d24e4b3-5f16-4d64-9b4c-b45ff860686f`, final Project three-test GREEN
`vrun-ba700fc1-af94-45f5-a213-db9e4c8f7543`. All these terminal GREEN runs have
zero skipped tests. The first compiler research run had a result-field mistake
and an incorrect guessed escaped summary; it is not product RED evidence.

Bare Name selection is not widened here. The current Q2-derived name catalog
deliberately omits ordinary TypeParameter records. The full designed semantic
token/role inventory must own future parameter name catalog selection rather
than changing navigation's catalog or adding a second ad hoc name lookup.
