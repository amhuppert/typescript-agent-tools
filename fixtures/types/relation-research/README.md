# Historical relation-limit research

This is the exact 2026-10-10 research capture reviewed in
[types-relation-limit-evidence.md](../../../docs/types-relation-limit-evidence.md).
`provenance.json` hashes the 67 research artifacts and pinned compiler inputs.
`report.md` preserves the original report at its recorded hash; the live document
adds the archive link.
The compiler source itself is not copied or modified. The failed primitive/fresh
object fixture attempts are retained alongside the successful natural families.

The original location was `.cc/temp/types-relation-limits/`. These scripts and
recorded paths preserve that layout; they are historical evidence, not a second
shipping oracle. Restore that scratch layout to replay them with the pinned
compiler environment. `setup.mjs` reads the then-current hook, whereas the
captured `research-oracle.mjs` and three Go hook files identify this exact run.
Changes to current shipping hooks therefore do not recreate historical evidence.

Public API fresh-demand results and native CLI whole-program diagnostics are
recorded separately. In particular, the allowance-under family succeeds under
fresh demand and diagnoses TS2859 during whole-program checking. No result here
proves general recovery causality, native Rust behavior, or completed delivery.

The shipping oracle incorporates reviewed generators and tests through its
single operation owners. This archive is excluded from its fixture manifest;
its own provenance owns the historical files.
