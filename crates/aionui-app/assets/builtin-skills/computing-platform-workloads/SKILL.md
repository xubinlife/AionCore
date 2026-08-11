---
name: computing-platform-workloads
description: Manage computing-platform workloads such as training, inference, and batch jobs. Use when the user wants to prepare, submit, inspect, modify, stop, or reason about workload configuration and execution.
---

# Computing Platform Workloads

Use the computing-platform MCP tools to work with platform workloads. Prefer current platform data over assumptions from the conversation.

## Workflow

1. Identify the workload or the workload configuration the user is referring to.
2. Read existing configuration, templates, quotas, and relevant resource availability before proposing changes.
3. For create or update requests, validate the important parameters that the MCP exposes, especially workload type, image/model, command, resources, storage, and target team or namespace.
4. Before submitting a new workload or changing an existing workload, summarize the effective configuration when any important parameter was inferred or changed.
5. For stop, restart, delete, or other high-impact operations, confirm the exact target when it is ambiguous and do not act on a guessed identifier.

## Rules

- Use MCP results as the source of truth for workload state and identifiers.
- Do not invent resource names, IDs, quotas, paths, images, or model versions.
- If the required computing-platform MCP tool is unavailable, explain what platform operation is needed instead of fabricating a result.
- Never expose Authorization headers, access tokens, API keys, or other credentials.
