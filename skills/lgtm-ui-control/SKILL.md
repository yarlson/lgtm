---
name: lgtm-ui-control
description: "lgtm local UI control skill. Use only when a selected PLAN.md phase changes browser, Electron, or local UI behavior and needs screenshot, accessibility, trace, or browser-driven evidence."
managed-by: lgtm
---

# lgtm UI Control

Use only when selected phase needs local browser, Electron, or UI verification.

Goal: evidence from actual UI surface, using repo-local tooling when available.

## Workflow

1. Identify UI surface and behavior under test.
2. Start app using repo's documented local command.
3. Prefer existing repo-native harnesses:
   - Playwright
   - Cypress
   - Storybook tests
   - browser scripts
   - Electron launch scripts
4. Select pages and controls by stable roles, labels, or app markers.
5. Capture before/after evidence when it proves selected phase.
6. Inspect console, network, trace, screenshot, or accessibility output only as needed.
7. Clean up servers, debug sessions, temp profiles, artifacts unless user asked to keep.

## Guardrails

Don't add Playwright, Cypress, or browser deps just for a probe unless selected phase requires.

Don't rely on stale selectors after navigation.

Avoid coordinate clicks unless fresh screenshot captured immediately before click.

Don't store screenshots, traces, HTTP bodies, or heap snapshots from sensitive workspaces unless needed and safe.

Don't hardcode ports, selectors, or scripts from another repo.

## Completion Criteria

UI control complete when UI behavior verified with local evidence, fixed within selected-phase scope, or blocked with clear reason.
