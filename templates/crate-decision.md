# ADR — Should this Rust project use more crates?

Status: PROPOSED | ACCEPTED | DEFERRED | REJECTED
Date:
Task/constraint:
Current repo/toolchain/target(s):

## 1. Evidence gathered

- Existing packages/modules/targets:
- Observed responsibility and ownership boundaries (files + functions):
- Internal/external dependency topology and feature concerns:
- Real consumers and tests (not hypothetical):
- Baseline compilation with comparable cold/warm configuration (or UNMEASURED):
- Current behavior invariants and risks:

## 2. Decision matrix

| Signal | Supports split / neutral / opposes split / unknown | Evidence, confidence, and cost |
|---|---|---|
| Cohesion and behavior ownership | | |
| Cross-boundary coupling and public API | | |
| Acyclic dependency direction | | |
| State ownership and data flow | | |
| Independent tests | | |
| Actual reuse/API stability | | |
| Compile time/features/dependencies | | |
| Lifecycle/build/platform requirements | | |
| Migration risk | | |

## 3. Options compared

| Option | Expected benefits supported by evidence | Costs/risks | Fit |
|---|---|---|---|
| A. Single crate, modules | | | |
| B. Single package, lib.rs + main.rs | | | |
| C. Multi-crate workspace | | | |
| D. Independent crates | | | |

## 4. Decision

**Decision:** KEEP | LIB-BIN | WORKSPACE | INDEPENDENT | DEFER

Why this option wins:
Why a simpler option is insufficient (only if splitting):
Why a more complex option is not justified:
Uncertainty/what to measure:

## 5. If splitting is justified

Proposed dependency DAG (arrows point to dependencies):
Owner of core data types and interfaces:
Explicit API to expose (`pub` surface):
Extraction order (pure move separate from behavior changes):
Tests / feature-target matrix / ABI / replay verification:
Build/performance evidence (before/after or UNMEASURED):
Rollback criteria:
Reviewer's independent assessment:
