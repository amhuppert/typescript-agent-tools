# Local dependency sources

These sources allow offline builds from this checkout. `.cargo/config.toml` patches the crates.io dependencies to these directories. This is a source subset for the production features this parser uses, not a full `cargo vendor` mirror supporting every upstream feature and test.

| Crate | Version | Origin |
| --- | --- | --- |
| nom | 8.0.0 | [rust-bakery/nom](https://github.com/rust-bakery/nom), tag `8.0.0` |
| serde, serde_core, serde_derive | 1.0.228 | [serde-rs/serde](https://github.com/serde-rs/serde), tag `v1.0.228` |
| serde_json | 1.0.145 | [serde-rs/json](https://github.com/serde-rs/json), tag `v1.0.145` |
| memchr | 2.7.6 | [BurntSushi/memchr](https://github.com/BurntSushi/memchr), tag `2.7.6` |
| ryu | 1.0.20 | [dtolnay/ryu](https://github.com/dtolnay/ryu), tag `1.0.20` |
| unicode-id-start | 1.5.0 | [oxc-project/unicode-id-start](https://github.com/oxc-project/unicode-id-start), commit `0647a2771289f39794468def321e51e46ea45273` |
| proc-macro2 | 1.0.107 | Existing local Cargo registry cache |
| quote | 1.0.47 | Existing local Cargo registry cache |
| syn | 2.0.119 | Existing local Cargo registry cache |
| unicode-ident | 1.0.24 | Existing local Cargo registry cache |
| itoa | 1.0.18 | Existing local Cargo registry cache |
| smallvec | 1.15.1 | [servo/rust-smallvec](https://github.com/servo/rust-smallvec), published crates.io archive |

[provenance.json](provenance.json) records upstream repository commits, file paths, and Git blob hashes for files fetched through the read-only GitHub connector. The 221 ordinary downloaded files were verified against those blob hashes; symlink entries preserve the resolved target contents. Serde's `src/core` symlink is expanded locally.

[smallvec-provenance.json](smallvec-provenance.json) records the published archive's SHA-256 and the original packaged file hashes. Only its local production manifest differs; unused optional and development dependencies are omitted. The `union` feature is enabled for the parser's inline fields.

Production Rust source files are upstream files. Local Cargo manifests omit development dependencies, workspace configuration, and unused optional feature dependencies to make offline resolution possible; they also relax vendor-only warnings from newer Rust versions. Each dependency's original license files remain in its directory. The parser package's MIT license does not replace those third-party licenses.

## TypeScript standard library declarations

[`typescript-7.0.2-lib`](typescript-7.0.2-lib) holds the 108 `lib*.d.ts` files of TypeScript 7.0.2, the declarations that `target`, `lib`, and `/// <reference lib>` select under the TypeScript 7 dialect. The TypeScript 5.6 dialect never reads them: it reads the installed compiler's own `lib` directory, by a catalog extracted from TypeScript 5.6.3 ([validation](../docs/validation.md#typescript-563-oracle)). They are data that find references reads at run time, not Rust sources: the built binary locates this directory through the checkout it was built from (`CARGO_MANIFEST_DIR`), so moving the checkout requires rebuilding, and a missing or unreadable file fails the load. `build.rs` watches `vendor`, so a change here gives the next build a new service identity.

| Data | Version | Origin |
| --- | --- | --- |
| `lib*.d.ts`, `LICENSE`, `NOTICE.txt` | 7.0.2 | `@typescript/typescript-darwin-x64` 7.0.2, published archive `typescript-darwin-x64-7.0.2.tgz`, `package/lib` and `package` |

The files are byte-identical to `internal/bundled/libs` of [microsoft/typescript-go](https://github.com/microsoft/typescript-go) at tag `typescript/v7.0.2` (commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`), which the native compiler reads from beside its executable. [`provenance.json`](typescript-7.0.2-lib/provenance.json) records the package, the archive's SHA-256, each file's SHA-256, the upstream tag and commit, and the library catalog (`LibMap` in declaration order and the target defaults) read from that commit's `internal/tsoptions/enummaps.go`, with that file's SHA-256. `node tools/vendor-typescript-libs.mjs` regenerates the directory from the pinned package and checkout, and `--check` verifies it without writing; [validation](../docs/validation.md#reference-semantics) lists the other checks.

The declarations are Apache-2.0 licensed by Microsoft. `LICENSE` and `NOTICE.txt` are the package's own and stay beside the files, each of which keeps its copyright header. The parser package's MIT license does not replace them.
