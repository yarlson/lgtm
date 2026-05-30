---
name: lgtm-technical-spike
description: "lgtm bounded technical spike skill. Use when a selected phase depends on unknown, unfamiliar, or version-sensitive framework, library, tool, runtime, or platform behavior. Produces implementation-relevant conclusions without drifting into broad research."
managed-by: lgtm
---

# lgtm Technical Spike

Use only when implementation or validation depends on unknown technical behavior.

Examples:

- unfamiliar framework APIs
- version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior
- unclear build or runtime constraints
- library behavior affecting implementation shape
- tool command needed for validation

## Workflow

1. State exact technical question.
2. Check repo-local evidence first:
   - manifests
   - lockfiles
   - config files
   - existing code
   - tests
   - installed versions
   - local help output
3. If local evidence insufficient, consult current official docs for specific tool or library.
4. Record only conclusions affecting selected phase.
5. Convert findings into implementation or validation implications.
6. Stop once phase can be implemented or validated safely.

## Output Shape

Use this concise structure in reasoning or final report:

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

Don't research adjacent features.

Don't upgrade dependencies unless selected phase explicitly requires it.

Don't add abstractions to hide uncertainty.

Don't cite unofficial sources when official docs available.

## Completion Criteria

Spike complete when selected phase has clear implementation or validation path.