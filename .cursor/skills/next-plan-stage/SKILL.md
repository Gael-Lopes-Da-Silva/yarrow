---
name: next-plan-stage
description: >-
  Create a git worktree, implement the next PLAN.md stage for a required Yarrow
  crate (yarrow-core, yarrow-cli, yarrow-fmt, yarrow-lsp, yarrow-runtime), commit,
  push, open a PR, then remove the worktree. Use when the user asks to implement
  the next stage, advance a crate plan, land the next PLAN gate, or run the
  worktree-stage-PR workflow.
---

# Next PLAN stage (worktree → PR)

End-to-end: isolate in a worktree, implement **one** next stage for a named crate, ship a PR, clean up the worktree.

## Required input

The user **must** name the crate. Do not default.

Valid crates (map to `crates/<name>/PLAN.md`):

- `yarrow-core`
- `yarrow-cli`
- `yarrow-fmt`
- `yarrow-lsp`
- `yarrow-runtime` (if it has a `PLAN.md`)

If the crate is missing or has no `PLAN.md`, stop and say so.

## Hard rules

- Follow [`AGENTS.md`](../../../AGENTS.md): `docs/GRAMMAR.md` is authoritative; update `PLAN.md` when the stage lands; no invented lifetime syntax; no tests unless the user asks; never use `—` in comments/docs.
- Implement **only** the next unfinished stage in that crate’s `PLAN.md` (first stage under Next / not yet Landed). Do not start later stages.
- Pass that stage’s **Gate** before committing.
- Commit, push, and open a PR only after the gate is green.
- Delete the worktree only after the PR URL exists and the push succeeded.
- Never force-push; never amend unless the user’s commit rules allow it.
- Do not push or open a PR from a dirty unrelated tree on the main checkout.

## Workflow

Copy and track:

```
Progress:
- [ ] 1. Identify next stage
- [ ] 2. Create worktree + branch
- [ ] 3. Implement stage + update PLAN.md
- [ ] 4. Verify gate
- [ ] 5. Commit
- [ ] 6. Push + open PR
- [ ] 7. Remove worktree
```

### 1. Identify next stage

1. Read `crates/<crate>/PLAN.md`.
2. Find the next actionable stage (typically the first under **Next** / Phase that is not marked done).
3. Summarize to the user in one short line: stage id, title, and gate. Then proceed unless they already constrained the stage.

### 2. Create worktree + branch

From the repo root (main checkout):

1. Ensure `main` (or the repo default branch) is clean enough to branch from; `git fetch` if needed.
2. Choose a branch and worktree path yourself (short, descriptive; include crate and stage id when useful). Example patterns: `stage-35-core`, `yarrow-fmt-stage-18`.
3. Create the worktree on a **new** branch from the default branch:

```bash
git fetch origin
git worktree add -b <branch> <path> origin/main
```

Use the repo’s actual default branch if it is not `main`.

4. Do all implementation, commits, and PR commands **inside that worktree path**.

### 3. Implement the stage

1. Follow the stage’s numbered tasks in `PLAN.md`.
2. Prefer minimal diffs that satisfy the gate.
3. When the stage is done, update that crate’s `PLAN.md` (Landed / Known gaps / Next) to match reality.
4. Cross-crate coordination: only touch another crate if the stage text explicitly requires it; keep the PR focused.

### 4. Verify the gate

Run what the stage gate and `AGENTS.md` require. Typical baseline:

```bash
cargo fmt --all
cargo check
cargo clippy
```

Plus any stage-specific commands or corpus checks named in the gate. Fix failures before committing.

### 5. Commit

Follow the user’s committing-changes protocol:

1. `git status`, `git diff`, `git log` (recent style) in parallel.
2. Stage relevant files only (no secrets).
3. Commit with a concise why-focused message via HEREDOC.
4. Verify with `git status`.

Do not commit until the gate passes.

### 6. Push and open PR

Follow the user’s creating-pull-requests protocol:

1. `git status`, `git diff`, tracking check, `git log` and `git diff main...HEAD` (or default base).
2. `git push -u origin HEAD`.
3. `gh pr create` with HEREDOC body:

```markdown
## Summary

<1-3 bullet points: stage id, what landed, why>

## Test plan

- [ ] Stage gate from PLAN.md (quote or paraphrase the checks)
- [ ] cargo fmt / check / clippy
- [ ] Any corpus or example commands from the gate
```

4. Return the PR URL to the user.

### 7. Remove the worktree

Only after the PR is created successfully:

```bash
git worktree remove <path>
```

If removal fails because of leftover state, fix or use `git worktree remove --force` only when the PR is already up and the worktree has no unique unpushed work.

Then `git worktree prune` if needed. Do **not** delete the remote branch.

## Report back

Keep it short:

- Stage implemented
- PR URL
- Worktree removed (path)
- Anything not done (blocked gate, skipped stretch goals)

## Failure handling

- Gate fails → fix in the worktree; do not open a PR; leave the worktree in place and report.
- Push/PR fails → leave the worktree; report the error and local branch name.
- Ambiguous “next” stage → ask which stage; do not guess across phases.
