# Specification Quality Checklist: Two physics modes (OMSI Classic and Project Depot Physics)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-11-08
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- FR-017 ("the code of each engine kept apart") and FR-019..FR-022 (model documents, conformance
  tests) describe how the work is organised rather than what a player sees. They stay because the
  maintainer asked for them explicitly; they are written without naming languages, modules or tools.
- `.bus` keys (`[depot_physics]`, `[cog]`) are domain terms for modders, not implementation details.
- The plan ([plan.md](../plan.md)) predates this spec; `/speckit-converge` or `/speckit-analyze` can
  check the two against each other.
- Re-validated after `/speckit-clarify` (session 2026-11-09, five questions): unconfirmed KA rows are
  built as "to validate" (FR-015), the engine is shown in the vehicle information panel and the
  launcher's bus list (FR-008a), and a crash is cleared with the Alt menu's "Clear the accident"
  button or by leaving the surroundings (FR-016); every item still passes.
- Re-validated after the second `/speckit-clarify` round (2026-11-09, five questions): the Classic AI
  lands in one phase (plan phase 8; the setting is now phase 9), no limit on loose AI vehicles
  (SC-007 also measures 10 loose), SC-006 measured on the test bends plus the maintainer's in-game
  comparison, a cleared AI bus goes back empty to its next trip (FR-016), and `[noDistanceCheck]`
  makes a vehicle count as near in both engines (FR-016c). The "loose and asleep" state was removed
  (a loose AI vehicle never sleeps). Every item still passes.
- Re-validated after the third `/speckit-clarify` round (2026-11-09, four questions): the Classic AI's
  scripts read a static sag computed at spawn about the origin (FR-015, KA-4, KA-7), the fallback
  `<name>.depot.cfg` if OMSI 2 or a fork rejects the `.bus` key (FR-009a), no career effect of a
  crash in this feature, and the surroundings centred on the camera ("the camera's surroundings").
  Every item still passes.
