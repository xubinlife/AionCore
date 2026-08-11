---
name: computing-platform-tickets
description: Work with computing-platform support tickets. Use when the user wants to list, inspect, summarize, create, update, or follow up on platform tickets and their current handling status.
---

# Computing Platform Tickets

Use the computing-platform MCP tools for ticket information and ticket operations.

## Workflow

1. Resolve the current user/team context when it affects which tickets are visible.
2. For queries, fetch the ticket list or ticket detail rather than relying on remembered status.
3. When summarizing a ticket, separate the reported problem, current status, latest response, unresolved items, and recommended next action.
4. Before creating or updating a ticket, verify the target ticket and the content that will be submitted.
5. For close, cancel, or other state-changing operations, make the intended state transition explicit before execution when there is ambiguity.

## Rules

- Preserve ticket IDs and status values exactly as returned by the platform.
- Do not claim that a ticket was created or changed until the MCP operation confirms success.
- Do not include secrets, Authorization headers, access tokens, API keys, or unrelated sensitive data in ticket content.
- If the required MCP capability is not available, state what action cannot currently be performed.
