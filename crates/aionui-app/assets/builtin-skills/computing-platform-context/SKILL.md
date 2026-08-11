---
name: computing-platform-context
description: Resolve the current computing-platform user and execution context. Use for account, team, namespace, role, permission, quota context, and questions whose answer depends on which platform scope the user is currently operating in.
---

# Computing Platform Context

Use the computing-platform MCP tools to resolve identity and scope before performing operations that depend on them.

## Workflow

1. Fetch the current account/user context when the user's request depends on identity or permissions.
2. Resolve the current or requested team, namespace, project, or equivalent platform scope.
3. For permission or quota questions, query the platform instead of inferring access from prior actions.
4. If multiple scopes are valid and the target cannot be inferred safely, ask the user to choose before a state-changing operation.
5. Reuse already-resolved context within the current task when it is still current, but refresh it when the user switches teams/scopes or the operation requires current authorization state.

## Rules

- Treat user/team IDs and permission data returned by MCP as authoritative.
- Do not silently switch team, namespace, or project context.
- Do not expose access tokens, Authorization headers, API keys, or other credentials when describing identity context.
- If context cannot be resolved, do not guess a scope for a modifying operation.
