---
name: lgtm-technical-spike
description: "lgtm bounded technical spike skill. Use when a selected phase depends on unknown, unfamiliar, or version-sensitive framework, library, tool, runtime, or platform behavior. Produces implementation-relevant conclusions without drifting into broad research."
managed-by: lgtm
---

# lgtm Technical Spike

Use this only when implementation or validation depends on unknown technical behavior.

Examples:

- unfamiliar framework APIs
- version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior
- unclear build or runtime constraints
- a library behavior that affects implementation shape
- a tool command needed for validation

## Workflow

1. State the exact technical question.
2. Check repo-local evidence first:
   - manifests
   - lockfiles
   - config files
   - existing code
   - tests
   - installed versions
   - local help output
3. If local evidence is insufficient, consult current official docs for the specific tool or library.
4. Record only conclusions that affect the selected phase.
5. Convert findings into implementation or validation implications.
6. Stop once the phase can be implemented or validated safely.

## Output Shape

Use this concise structure in your reasoning or final report:

```md
## Technical Spike Result

Question: ...
Local evidence: ...
External evidence, if used: ...
Conclusion: ...
Impact on this phase: ...
Remaining uncertainty: ...
```

## Guardrails

Do not research adjacent features.

Do not upgrade dependencies unless the selected phase explicitly requires it.

Do not add abstractions to hide uncertainty.

Do not cite unofficial sources when official docs are available.

## Completion Criteria

The spike is complete when the selected phase has a clear implementation or validation path.
