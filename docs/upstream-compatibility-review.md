# Upstream compatibility review (2026-11-15)

Base: `fe5db0857d0fe382806f81b27436579806fe1136`, PR #1.
Upstream: [mkxp-z/mkxp-z](https://github.com/mkxp-z/mkxp-z).

Scope: screened all 166 open items (105 issues, 61 pull requests), then
investigated the relevant deterministic Rust-core paths. This is not a claim
that all upstream reports or games were reproduced. No upstream changes were
made. The retained C++ player was not changed or tested.

## Fixed

- [Unicode title report #373](https://github.com/mkxp-z/mkxp-z/issues/373)
  motivated configuration decoding tests. A distinct, confirmed Rust bug decoded
  valid UTF-16 surrogate-pair escapes as two replacement characters. This affects
  titles and other string settings (including RTP paths). Literal UTF-8 and
  escaped non-BMP characters now agree. The test failed before the fix.
  This does **not** establish that the upstream Pokémon example has this cause.
- [Archive seek PR #381](https://github.com/mkxp-z/mkxp-z/pull/381): the Rust
  `advance_n` helper also had a 30-entry jump table and panicked for counts at
  least 2^30. Reduce the count modulo the LCG period, as upstream proposes.
  An independent affine-exponentiation test failed before the fix and now
  covers 2^28, 2^30, 2^31 and u32::MAX steps without allocating huge archives.
  Current public reads are bounded by 32-bit archive sizes, so the tested helper
  panic is defensive hardening, **not** a demonstrated externally reachable
  crash through a valid archive. Rust does not expose upstream's backward-seek API.

## Verified differences / regression coverage

| Upstream | Rust-core conclusion |
| --- | --- |
| [#417](https://github.com/mkxp-z/mkxp-z/pull/417), pathological archive names | No recursive path cache. Empty/dot/dot-dot records in v1/v2/v3 do not recurse; valid entries remain readable. |
| [#382](https://github.com/mkxp-z/mkxp-z/pull/382), [#383](https://github.com/mkxp-z/mkxp-z/pull/383), endian serialization | Explicit little-endian conversion, no C++ type punning. Added fixed wire-byte table fixture. No big-endian-machine run. |
| [#315](https://github.com/mkxp-z/mkxp-z/pull/315), [#319](https://github.com/mkxp-z/mkxp-z/issues/319), new file lookup | No stale directory cache: mounted directories see newly created files. Tested after an earlier lookup. |
| [#92](https://github.com/mkxp-z/mkxp-z/issues/92), [#93](https://github.com/mkxp-z/mkxp-z/pull/93), path precedence | Earlier directory mounts win over archives, including extension supplementation (tested). NFC/NFD equivalence is **not** implemented. |
| [#241](https://github.com/mkxp-z/mkxp-z/issues/241), XP tile wrapping | Already wraps both axes. Added pixel-level tests at negative and multiple-map offsets. |
| [#398](https://github.com/mkxp-z/mkxp-z/issues/398), [#406](https://github.com/mkxp-z/mkxp-z/pull/406), small VX window borders | Current renderer uses XP skin, not WindowVX. Added XP short/narrow-window edge tests; do not claim VX compatibility. |
| [#311](https://github.com/mkxp-z/mkxp-z/pull/311), Table out-of-bounds writes | Typed i16 writes are bounds checked; existing tests pass. Ruby value coercion is outside this core. |
| [#271](https://github.com/mkxp-z/mkxp-z/pull/271), initial screen size | Graphics uses supplied dimensions directly, not separate C++ scRes state. Native mouse mapping is untested. |
| [#314](https://github.com/mkxp-z/mkxp-z/pull/314), [#339](https://github.com/mkxp-z/mkxp-z/issues/339), giant bitmaps | Test confirms >8192 dimensions return errors before allocation. This is not mega-bitmap feature compatibility. |

## Still out of scope / unverified

- MRI/game execution, SDL/OpenGL platform state, FreeType metrics, native audio,
  networking, Switch/libretro ports and native build/dependency PRs do not run
  through this std-only Rust core.
- [#359](https://github.com/mkxp-z/mkxp-z/pull/359) radial blur is not implemented.
- [#227](https://github.com/mkxp-z/mkxp-z/issues/227) and
  [#228](https://github.com/mkxp-z/mkxp-z/issues/228) game rendering cannot be
  declared fixed without their games and MRI. Per-row tilemap/sprite interleaving
  remains absent.
- [#407](https://github.com/mkxp-z/mkxp-z/pull/407) motivates a future actual-game
  harness; these Rust tests are not a replacement for that acceptance test.

## Checks on the resulting code

- 50 tests pass (44 unit + 6 integration); `cargo fmt --check` and
  `git diff --check` pass. Two-frame software demo completes.
- Strict Clippy on Rust 1.99.1 still fails on six pre-existing lint findings:
  `manual_is_multiple_of` (2), `chunks_exact_to_as_chunks` (3), and
  `manual_contains` (1). No suppression or unrelated MSRV-changing cleanup.
- No full game, GPU, device audio, Windows/macOS or big-endian runtime acceptance.
- Existing workflow is for C++ builds, not a Rust CI acceptance gate.
