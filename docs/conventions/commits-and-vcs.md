# Commits + VCS

## Conventional commits

`feat:` / `fix:` / `chore:` / `refactor:` / `test:` / `docs:` — **no scopes**.

## Solo-to-main

Commits push to `main` directly. Feature branches are optional. The global `wnmitch/<name>` convention (from `~/.claude/CLAUDE.md`) does **not** apply here.

## jj, not raw git

Use `jj` for all VCS operations. It is git-compatible — the repo on disk is a git repo, and tools like `gh` work normally. But the working-copy model differs from git, so raw `git checkout` / `git reset` will fight `jj`.

Multi-workspace caveat: I often run parallel sessions in separate `jj` workspaces against the same repo. In `jj log`, `@` is *this* workspace and `<name>@` entries belong to other sessions — don't modify them, never `jj abandon` them. Don't move shared bookmarks like `main`; create a task-scoped bookmark and push with `jj git push --bookmark <name>`. If jj reports this workspace is stale, run `jj workspace update-stale`.

## Ticket lifecycle is script-driven

Never hand-edit ticket frontmatter, hand-move files between `tickets/` and `landed/`, hand-clear `blocked-by` entries, or hand-regenerate `docs/open-work.md` when `just land` or `just open-ticket` covers the operation.

See [`docs/workflow/ticket-lifecycle.md`](../workflow/ticket-lifecycle.md) for the full lifecycle.

## Design docs

`docs/systems/` — one stub per tunable system. Auto-generated status: `docs/wiki/systems.md`. Any change to `SimulationPlugin::build()` regenerates `docs/wiki/systems.md` (`just wiki`) in the same commit.
