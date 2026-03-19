+++
title = "Abstractions Under Forge"
subtitle = "Architectural Argument"
version = "0.1.0"
date = 2026-03-19
status = "Draft"
+++

# Abstractions Under Forge

## Premise

Forge has two kinds of data in Git.
They require different abstractions.
This document argues for what those abstractions are, where the boundaries fall, and what should not be abstracted prematurely.


## Two Kinds of Data

### Metadata

Annotations on existing Git objects.
A comment on a blob, an approval on a patch-id, a check result on a commit, a relational link between two things.
The annotated object exists independently; the metadata describes it.

This is `git-metadata`.
It extends Git's notes to tree-structured data, allowing arbitrary metadata to be attached to any object without modifying history.
It handles annotations, relational links, fanout by OID or patch-id, and bidirectional link consistency.

### Entities

Standalone refs with their own lifecycle.
An issue, a review, a contributor record, a policy document, a queue, a release.
These are not metadata on anything.
They are independent objects with sequential IDs, commit history as an audit log, and tree-structured state.

Entities are collections of structured records stored as refs.
They share a common pattern:

- Ref → commit → tree of blobs.
- Sequential ID assignment via counters.
- Append-only history: each mutation is a new commit.
- Prefix scan for listing.
- Namespace scoping.

This is not `git-metadata`.
It is also not raw `git2`.
It is a distinct concern.


## The Layering

```text
1. git              — objects, refs, transport
2. git-metadata     — structured metadata on git objects
3. forge            — schema, workflows, hooks, UX
```

There is no `git forge` CLI.
There never needed to be.
`git-metadata` is the Git extension.
`forge` is a consumer of it.

`forge` depends on `git2` for entity CRUD (standalone refs, trees, commits) and `git-metadata` for annotations and links.
Two existing dependencies.


## Where Configuration Lives

### Refs, Not the Worktree

Policy, namespaces, hooks, and contributor roles are repo-level state, not code-level state.
They govern the repository, not a snapshot of it.
A branch could delete a config file in the worktree.
That is incoherent for policy.

Repo-level configuration lives in refs:

```text
refs/forge/meta/config → commit → tree
├── policy.toml
├── namespaces.toml
└── hooks/
    ├── id-assign
    └── pre-issue-create
```

Changing policy requires a signed commit to this ref, governed by the current policy.
No branch can delete it.
No fork accidentally diverges it.

### The One Exception

Check definitions belong in the worktree:

```text
.forge/
└── checks/
    ├── build.toml
    └── test.toml
```

The check that runs must match the code being checked.
Everything else is repo-level state.


## Namespaces

Multiple teams in one repo use namespaced entity IDs.
The namespace is structural, not display-only.

### Counter Refs

Per-namespace counter refs eliminate cross-team serialization:

```text
refs/forge/meta/counters/TEAM
refs/forge/meta/counters/PLAT
```

Each namespace is fully independent: its own counter ref, its own ref subtree, no contention across teams.

### Ref Paths

The namespace segment propagates everywhere an entity ID appears:

```text
refs/forge/issue/TEAM/12
refs/forge/review/PLAT/7
refs/forge/comments/issue/TEAM/12
refs/metadata/links/issues/TEAM/12/
```

### CLI Ergonomics

User-level default namespace via Git config:

```sh
git config forge.namespace TEAM
```

Resolution order: explicit argument → `forge.namespace` → error if namespaces are configured and neither is set.

```sh
forge issue new "Fix the auth bug"       # uses default namespace
forge issue list PLAT                     # explicit override
forge issue list --all                    # cross-namespace
```

### Policy

```toml
[namespaces.TEAM]
roles = ["team-dev", "maintainer"]
entity_types = ["issues", "reviews"]

[namespaces.PLAT]
roles = ["platform-dev", "maintainer"]
entity_types = ["issues", "reviews"]
```

Cross-namespace references are permitted through relational metadata.
Namespaces are organizational boundaries, not isolation boundaries.


## Hooks

Hooks live in `refs/forge/meta/config`, not the worktree.
`forge` reads and executes hooks.
The plumbing libraries never run hooks.

### Separation of Concerns

Hooks handle **policy decisions**: ID format, validation, pre/post actions, custom fields.
These are the extension points where teams legitimately differ.

`forge` enforces **structural invariants**: atomic ref writes, correct tree structure, bidirectional link consistency, comment threading DAG structure.
These are what make forge repos interoperable.

A broken hook cannot corrupt the data model because the plumbing never runs hooks.
Only the porcelain opts into extensibility.

### Custom ID Assignment

The `id-assign` hook is the mechanism for custom IDs like `TEAM-1`.
The hook receives entity type and namespace; it returns the assigned ID.
`forge` calls the hook, then writes the ref in the canonical format.
The hook influences the ID; `forge` owns the ref write.


## Adoption Path

### The LLM Context Argument

With forge refs in the repo, an LLM reads issues, review discussions, approval status, and comment threads the same way it reads source files.
Clone the repo, get the entire project context.
No API, no auth, no rate limits.

### Priority Order

1. **`forge` library crate** — read entities, comments, approvals, links from refs.
2. **MCP server** — expose forge reads as tools.
   Works with ACP agents, Claude Code, Codex, anything that speaks MCP.
3. **CLI** — thin porcelain over the library for humans.
4. **GitHub sync** — backfill existing project data into refs.

MCP is the right integration layer.
ACP agents already consume MCP servers.
One MCP server covers Zed, JetBrains, Neovim, and every agent that speaks MCP.

The GitHub App and frontend come after the local experience is compelling.
People switch when `forge` makes their daily workflow better, not when their data is mirrored into refs they never look at.


## The Entity Abstraction Question

Issues, reviews, contributors, queues, releases, and policy all share the entity pattern: ref → commit → tree, sequential IDs, append-only history, prefix scan, namespace scoping.

This is a real abstraction.
It is not `git-metadata` (wrong concept) and not raw `git2` (too low-level, repetitive boilerplate).
It would be something like `git-entity` or `git-collection`: a library for managing named, versioned, structured records as Git refs.

### Recommendation: Do Not Extract Yet

Build issues and reviews in `forge`.
When the repeated patterns are concrete — not speculative — extract.
The differences between entity types may dominate the similarities.
Premature extraction creates an abstraction that serves no entity well.

The signal to extract: writing the third or fourth entity type and copy-pasting the same ref/commit/tree boilerplate.
Not before.
