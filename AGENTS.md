# AGENTS.md

Private repo (Forgejo `Tyler/ontos`). No GitHub mirror.

## Rules

- Conventional commits (`feat:`, `fix:`, `chore:`, `docs:`).
- No session notes, handoff docs, or transient context as tracked files.
  Design lives in `docs/`; scratch lives in gitignored `notes/`.
- The determinism contract in README is load-bearing. No wall-clock reads in
  tick paths, no ambient randomness, no fast-math. Every PR keeps replay
  bit-identical or documents why the contract changed.
- simval stays external. Adapters and reference implementations live here.
- Read docs/DESIGN.md before structural changes; update it when they land.
