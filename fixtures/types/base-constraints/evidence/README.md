# Historical native base-constraint evidence

These exact research files preserve fresh TypeScript 7.0.2 sessions from
`.cc/temp/types-base-constraints`. The provenance manifest hashes all 51
historical artifacts, three captured loader dependencies and the compiler
binary/package/source pins. They are evidence captures, not fresh oracle
fixture inputs or a production compiler backend.

The copied scripts retain their original worktree/cache paths. Reproduction
requires the historical scratch layout, the captured loader bytes and pinned
clean compiler installation; running them from this archive without restoring
that layout changes inputs. Raw answers preserve exact replay input hashes,
already accumulated diagnostics, before-summary and after-base observations.
No counter, limit, cache or native threshold was seeded or changed.

`answers.json` contains distinct-symbol parameter chains: counts 9/10/11
reach actual stack depths 8/9/10; count 50 reaches depth 49 and a literal base;
counts 51/52 stop at depth 50 with NoConstraint while the guarded/raw declared
constraint remains T1. Native noLib initialization diagnostics are retained,
with no new circular-constraint diagnostics from that safety approximation.

`bounded-repeat-answers.json` contains the natural bounded conditional family:
N=5 reaches depth 9 and `number | "done"`; N=6/7 stop at depth 10 with a repeated
recursion identity and NoConstraint. `repeat-answers.json` preserves unsuccessful
alternative families, including genuine circular outcomes. Their observations
must not be substituted for the bounded family's normal safety branch.

Public registered native controls also use marker-bearing sources under
`../limits`; they are separate exact inputs. The product's current established
parameter/union/structural consumers are proved by the three Project tests.
Symbolic operator factories, mapped fresh-parameter cache protocols and resource
gates remain required implementation work.
