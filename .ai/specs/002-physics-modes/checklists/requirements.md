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
