# Missing globalThis properties

Pinned TypeScript 7.0.2 at `2bd066d87f5bafd315be9f40889d0a60b9e58e0b` is the behavior source (the harness also verifies the runtime version).

The checked property access owner returns a newly supplied ordinary `any` for absent globalThis properties, including when noImplicitAny suppresses the diagnostic. The product records that consumed fallback as ErrorType recovery. In contrast, an existing Any receiver preserves its own type without new recovery. Raw lexical let/const/class entries are distinct from absent and type-only entries: the BlockScoped branch displays the receiver regardless of noImplicitAny. Var/function and enum controls establish the native exposed global inventory.

The hook uses the existing zero-demand diagnostic display observer. Before-summary display events and diagnostic membership are captured independently from the eventual summary. Each marker is a fresh checker request; repeatMissing is an ordinary expression-local control and does not assert a cross-request warm cache.
