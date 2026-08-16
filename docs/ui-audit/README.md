# `git-forge-ui` UX/UI cleanup audit

Six standalone jobs.
Each `phase-N.md` is a self-contained prompt for a fresh session — copy the file's contents in, nothing else.
Phases never read each other; they communicate only through `findings.md` in this directory.

Read `../plan-ui-audit.md` for the rationale, the reusable scaffolding, and the environment facts.
Agents running a phase do **not** need it.

| Phase | Job | Depends on |
|---|---|---|
| [0](phase-0.md) | Capture harness | — |
| [1](phase-1.md) | Copy and nomenclature | — |
| [2](phase-2.md) | Spacing and typography | 0 |
| [3](phase-3.md) | Visual markers, states, affordances | 0 |
| [4](phase-4.md) | Fix pass | 1, 2, 3 |
| [5](phase-5.md) | Regression guard (optional) | 4 |

Phases 0 and 1 are independent and can run in parallel.
Phase 1 needs no tooling at all, so it is the cheapest place to start.
