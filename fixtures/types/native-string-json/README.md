# Cooked JSON text and strict manifests

The 27 native projects compare three consumers across lone high/low, each of
the five line continuations, adjacent paired escapes, and a written scalar.

Configuration uses the TypeScript scanner: continued halves name no existing
scalar or replacement-character file, producing TS6053. Adjacent paired escapes
and a written scalar load the actual supplementary-character file.

JSON source uses the same scanner. A scalar imported key has only its first
declaration when the second key contains sentinel halves. Paired/scalar controls
retain both duplicate declarations. This exposes key identity without relying
on display text that could replace invalid bytes.

Package manifests use native strict JSON parsing. Lone surrogates and line
continuations make the manifest malformed, so native resolution takes the
index.d.ts fallback. Paired/scalar controls select the exact types file. The
replacement-character file exists in every case and is never selected. This
reader remains separate from the TypeScript scanner decoder.
