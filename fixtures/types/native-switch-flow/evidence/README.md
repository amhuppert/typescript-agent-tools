# Frozen native Switch controls

The parent fixtures and `tools/type-switch-flow-oracle.test.mjs` are executable
compiler controls. The files here retain historical evidence and are excluded
from the fresh oracle fixture manifest.

`research-*` preserves the original scratch sources, configs, generator and all
86 raw fresh API answers. Their config paths intentionally name the historical
scratch location. Those answers include selected source hashes and effective
compiler options; they do not contain a complete input-filesystem manifest or
all metadata hashes. Later inventory cannot prove historical metadata identity.

`captured-tooling/` retains the six exact local modules used by the hash-frozen
registered formal replay. Reproduction restores those files to their original
`tools/` locations and uses the formal fixtures at their original paths; the
historical copies are not a second executable oracle. The compiler is native
TypeScript 7.0.2, source commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.
`installed-api-inventory.json` records all installed API-package bytes at replay
capture; `replay-provenance.json` records the native binary and source anchors,
and unchanged before/after local inputs. It distinguishes package inventory from
per-request compiler-read metadata. This noLib API slice does not invoke the Go
compiler-hook overlay or a semantic diagnostics pass.

`registered-replay.json` records the exact registered command output. Its durable
terminal run is `vrun-75bc6c27-e290-48db-aaad-a833c35170f8`, three tests passed,
zero skipped. `capture.mjs` preserves the capture procedure. `provenance.json`
hashes this evidence tree except itself. These observations authorize native
Switch implementation tests, not a claim that the product's complete semantic,
checking-history, cache or resource obligations have passed.
