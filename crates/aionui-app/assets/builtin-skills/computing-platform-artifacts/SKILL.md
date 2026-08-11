---
name: computing-platform-artifacts
description: Work with computing-platform datasets, models, checkpoints, files, and other artifacts. Use to discover, inspect, compare, register, manage, or reference data/model artifacts used by platform workloads.
---

# Computing Platform Artifacts

Use the computing-platform MCP tools to locate and manage datasets, models, checkpoints, files, and other platform artifacts.

## Workflow

1. Determine which artifact type the user means and resolve the correct platform scope.
2. Search/list before assuming an artifact identifier, path, version, or ownership.
3. Read artifact metadata and status before using it in a workload or recommending a modification.
4. When several versions or similarly named artifacts exist, show the relevant differences and use an exact identifier for subsequent operations.
5. For register, update, delete, publish, or other state-changing operations, verify the target and material fields before execution.

## Rules

- Do not invent dataset/model names, versions, paths, sizes, checksums, or readiness state.
- Do not delete, overwrite, or publish an artifact based only on a fuzzy name match.
- Prefer platform-managed identifiers over manually reconstructed paths.
- Never expose Authorization headers, access tokens, API keys, signed credentials, or other secrets.
