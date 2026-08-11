---
name: computing-platform-resources
description: Query computing-platform compute resources and quotas. Use for GPU/CPU/memory availability, allocations, usage, quotas, namespaces, resource pools, and resource-related scheduling questions.
---

# Computing Platform Resources

Use the computing-platform MCP tools to answer resource and quota questions from live platform data.

## Workflow

1. Resolve the relevant team, namespace, cluster, resource pool, or other scope before comparing capacity or quota.
2. Fetch the resource inventory, availability, allocation, usage, or quota data needed for the question.
3. Preserve units and resource types exactly; make conversions explicit when useful.
4. When explaining why a workload cannot be scheduled, correlate the workload request with available capacity, quota, policy, and scheduler information rather than assuming GPU shortage.
5. When recommending a resource request, distinguish currently available capacity from configured quota and historical usage.

## Rules

- Do not invent GPU models, quantities, quota values, utilization, or availability.
- State the scope and timestamp/context of resource data when it matters.
- Do not mutate quotas or resource configuration unless the user explicitly asks and the MCP provides an appropriate operation.
- Never expose Authorization headers, access tokens, API keys, or other credentials.
