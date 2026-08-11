---
name: computing-platform-tasks
description: Inspect and troubleshoot computing-platform tasks. Use for task lists, task status, events, logs, failure diagnosis, lifecycle operations, and questions about why a task is pending, failed, or stopped.
---

# Computing Platform Tasks

Use the computing-platform MCP tools to inspect task state and diagnose task failures from current evidence.

## Diagnosis workflow

1. Resolve the task by exact ID or by listing recent tasks when the user gives only a description.
2. Fetch task detail and current state first.
3. If the task is abnormal, collect the relevant events, logs, scheduling/resource information, and other diagnostics exposed by the MCP.
4. Build an evidence chain: observed state -> relevant error/event/log -> likely failure stage -> likely cause -> recommended next action.
5. Distinguish confirmed facts from hypotheses. Do not treat a guessed root cause as confirmed.

## Lifecycle operations

- Before stop, restart, retry, or delete operations, verify the exact task and explain the requested transition when needed.
- Do not submit a replacement task unless the user requested it and the effective configuration is known.

## Rules

- Do not fabricate logs, events, task IDs, exit codes, scheduler reasons, or timestamps.
- Prefer targeted log/event reads over dumping unrelated output.
- If the MCP cannot provide a required diagnostic, say which evidence is missing.
- Never expose Authorization headers, access tokens, API keys, or other credentials.
