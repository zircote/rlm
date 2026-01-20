---
adr_paths:
  - docs/adr

default_format: structured-madr

numbering:
  pattern: "###"
  start_from: 1

statuses:
  workflow:
    - proposed
    - accepted
    - deprecated
    - superseded
  allow_rejected: true

git:
  enabled: true
  auto_commit: false
  commit_template: "docs(adr): {action} ADR-{id} {title}"
---

# rlm-rs Architecture Decision Records

## Decision Process

- ADRs are proposed via pull request
- Team review required before acceptance
- Significant architectural changes should be documented

## Conventions

- Use Structured MADR format with YAML frontmatter
- 3-digit numbering (001, 002, 003...)
- Status workflow: proposed → accepted → deprecated → superseded
