# Snapshot: Git Stash-Based Workflow Checkpoints

## Overview

**Snapshotter** (`internal/snapshot/`) creates non-disruptive git stash snapshots at each workflow step, enabling recovery and audit trails without interfering with working tree operations.

## Capabilities

**Snapshot.Capture()** creates a stash with:

- Current working tree state (all changes, staged and unstaged)
- Untracked files included
- Original index state preserved (staged files remain staged after snapshot)
- Message label with task, step, and operation name

**Return values**:

- `true` — snapshot created (working tree had changes)
- `false` — working tree was clean (nothing to snapshot)
- `error` — git operation failed (usually not a git repo)

## Integration with Workflow Runner

**Optional feature** — disabled by default to avoid test side effects.

**Enable via option**:

```go
runner := workflow.NewRunner(executor, config,
    workflow.WithSnapshotter(snapshot.New(workDir)),
)
```

**Behavior during iteration**:

- Captures snapshot after each step executes
- Skips snapshots for Commit steps (tree is clean after commit, creates empty stash)
- Logs snapshot result: "snapshot saved" or "snapshot skipped: <error>"
- Non-fatal: snapshot errors do not halt iteration

**Snapshot messages** follow pattern:

```
snap: <task-label> step <N>/<TOTAL> — <step-name>
```

Example: `snap: TASK1 step 3/10 — Lint & test`

## Use Cases

1. **Debugging**: Review stash list to see state at each step
2. **Recovery**: Restore specific step snapshot if iteration fails
3. **Audit trail**: Git reflog shows when snapshots were created
4. **Testing**: Verify intermediate outputs without modifying files

## Implementation Details

**Snapshotter.Capture()** workflow:

1. Save current git index state (`write-tree`)
2. Stage all files including untracked (`add .`)
3. Create stash without modifying working tree (`stash create`)
4. Restore original index state (`read-tree` or `reset`)
5. Store stash in reflog (`stash store`)

This preserves the exact prior state: intentionally-staged files remain staged, untracked files revert to untracked.

## Git Interactions

- Requires valid git repository
- Uses `git write-tree`, `git add`, `git stash create`, `git read-tree`, `git stash store`
- Does not modify working tree or create new commits
- Snapshots accessible via `git stash list`
