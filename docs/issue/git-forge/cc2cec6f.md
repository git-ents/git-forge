---
id: cc2cec6f
repo: git-forge
ref: refs/forge/issue/cc2cec6f
title: "rm silently and irrecoverably destroys an entity, which is almost never what a forge user wants and is not what close means"
status: open
labels: ["cli-ux", "medium"]
reporters: ["Claude:claude-opus-5"]
assignees: []
updated: 2026-07-28T10:41:42+09:00
revisions:
  - commit: cc2cec6f936675239cbeee7c859e48ef278cd774
    date: 2026-07-28T10:41:42+09:00
---

# rm silently and irrecoverably destroys an entity, which is almost never what a forge user wants and is not what close means

**What**

`issue rm <id>` and `review rm <id>` delete the ref outright. No confirmation
prompt, no output, exit 0. The commits become unreachable and the reflog does
not survive, so the entity and its entire history are gone at the next `gc`.

**Where**

- `crates/git-forge/src/main.rs:315` -- `IssueCommand::Rm`, deletes and prints nothing
- `crates/git-forge/src/main.rs:903` -- `ReviewCommand::Rm`, same
- `crates/gix-forge/src/lib.rs:161` -- `Issue::delete`

**Reproduction**

    git-forge issue rm 28f76833
                                    # no output, exit 0
    git for-each-ref refs/forge/issue/28f76833
                                    # gone
    git reflog refs/forge/issue/28f76833
    fatal: ambiguous argument ... unknown revision

**Why it matters**

`rm` is the wrong verb for the wrong operation. In a forge, the thing a user
wants ninety-nine times out of a hundred is `close`, and `--status closed`
already exists -- so the destructive operation is the discoverable one with the
short familiar name, and the safe one is a flag on `edit`. That is backwards.

It is also inconsistent with the rest of the design. The entire pitch is that
history is preserved and auditable because it is Git. `rm` is the one place the
tool throws history away, and it does so without asking.

And it is a demo hazard in the most literal sense: `rm` is muscle memory,
`git-forge issue rm` reads as harmless if you think of issues as rows, and
there is no undo.

**Options**

- Option A -- Rename to `issue close` / `issue reopen` as the primary verbs
  (thin wrappers over `--status`), and keep deletion as `issue delete --force`
  with a typed confirmation. Matches `gh issue close` muscle memory. Trade-off:
  none really, beyond an alias to keep.
- Option B -- Keep `rm` but make it a tombstone: write a final commit marking
  the entity deleted rather than removing the ref, so the history survives and
  the deletion itself is auditable and merges. Trade-off: deleted entities still
  cost a ref, and `list` needs to filter them.
- Option C -- Keep `rm` destructive but require `--yes` off a TTY and confirm
  on one, and print what was deleted. Trade-off: minimum change, does not fix
  the vocabulary problem.

**Recommendation**

Option A for the vocabulary and Option B for the mechanism -- a forge built on
an append-only substrate should not have a verb that makes data disappear, and
a tombstone is both safer and more in keeping with the thesis. Reasonably high
confidence, though I can see an argument that spam and accidental issues need
a real delete; if so, that is Option C on top, gated behind `--force`.

Worth noting alongside this: `git-forge issue new` prints only a bare hex id on
success and nothing else. That is correct for scripting and I would not change
it -- but the pair of a create that says nothing and a delete that says nothing
means the tool is silent in both directions at the two moments a human most
wants confirmation. The middle path is id on stdout, one summary line on
stderr, which keeps pipelines clean and gives humans a receipt.
