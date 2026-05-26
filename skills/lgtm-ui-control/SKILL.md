---
name: lgtm-ui-control
description: "lgtm local UI control skill. Use only when a selected PLAN.md phase changes browser, Electron, or local UI behavior and needs screenshot, accessibility, trace, or browser-driven evidence."
managed-by: lgtm
---

# lgtm UI Control

Use this only when the selected phase needs local browser, Electron, or UI verification.

The goal is evidence from the actual UI surface, using repo-local tooling when available.
`PLAN.md` is immutable after `/finish`; record UI evidence, blockers,
verification, and status notes in root-level `PLAN_STATUS.md`, creating it if
it is missing. Use `lgtm-plan-update` only for an exceptional selected-phase
contract defect.

## Workflow

1. Identify the UI surface and behavior under test.
2. Start the app using the repo's documented local command.
3. Prefer existing repo-native harnesses:
   - Playwright
   - Cypress
   - Storybook tests
   - browser scripts
   - Electron launch scripts
4. Select pages and controls by stable roles, labels, or app markers.
5. Capture before/after evidence when it proves the selected phase.
6. Inspect console, network, trace, screenshot, or accessibility output only as needed.
7. Clean up servers, debug sessions, temp profiles, and artifacts unless the user asked to keep them.

## Guardrails

Do not add Playwright, Cypress, or browser dependencies just for a probe unless the selected phase requires it.

Do not rely on stale selectors after navigation.

Avoid coordinate clicks unless a fresh screenshot was captured immediately before the click.

Do not store screenshots, traces, HTTP bodies, or heap snapshots from sensitive workspaces unless needed and safe.

Do not hardcode ports, selectors, or scripts from another repository.

Do not edit `PLAN.md` for UI findings, blockers, verification summaries, or
future-work notes after `/finish`; those notes belong in `PLAN_STATUS.md`.

## Completion Criteria

UI control is complete when the UI behavior is verified with local evidence, fixed within selected-phase scope, or blocked with a clear reason.
