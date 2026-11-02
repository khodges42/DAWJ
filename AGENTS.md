# Contributor automation

- Read `README.md`, `docs/development/README.md` and the relevant architecture
  contract before changing behavior. Use the pinned toolchain/lockfile.
- The audio callback owns scheduling/DSP. No GUI work, blocking locks, I/O,
  allocation or unbounded processing belongs there. Reuse shared semantic actions
  and coherent publications; keep both frontends functional.
- Run formatting, check, Clippy, workspace tests and relevant Node/Python checks.
  Review dependency licensing before changing the locked graph.
- Frontend audio tests require a unique named HyperX output by default. Never
  fall back to another device. Do not intentionally run heavy apps/builds alongside
  latency trials; record host-load and hardware limitations.
- Put transient implementation reports, handoffs and scratchpads in the local,
  ignored `.dev-notes/codex/`. Keep public guides/contracts and selected reusable
  engineering evidence in `docs/`; update them when permanent behavior changes.
- Preserve user design notes and untracked plans. Do not delete or stage them as
  incidental cleanup. Do not make a fresh clone depend on ignored notes.
- Make focused commits; do not push, merge, switch the default frontend or start
  another milestone without user authorization.
