# RutAgentIA — captain loop

This is the durable operating contract for recurring agent cycles. It is
intentionally runner-agnostic: a Codex heartbeat, CI job, or local supervisor
may invoke it, but a stopped session must never be reported as an active team.

## Cycle

1. The captain reads `teamwork_op.txt`, `CLAUDE.md`, and the current git state.
2. Before fan-out, the captain consolidates finished work: inspect diffs,
   close stale workers, and keep the number of finished-but-unmerged changes
   within the WIP limit in `CLAUDE.md`.
3. Dispatch at most one bounded task per lane. Every task names its files,
   required gate, expected evidence, and a stop condition.
4. Workers edit only their declared scope and return changed paths, tests,
   blockers, and any multi-rubro assumption.
5. The captain reviews the combined diff, runs the strongest available gates,
   updates `teamwork_op.txt`, and either integrates the cycle or records a
   concrete blocker. Never overwrite another lane to resolve a collision.

## Suggested heartbeat

Use a 30-minute heartbeat while there is active WIP. Each tick should inspect
state first; it may dispatch work only when the previous cycle is consolidated.
Use a longer heartbeat (2–4 hours) when waiting on network, CI, or founder
input. Stop the heartbeat when the backlog is empty or the user asks to stop.

## Required handoff

Every cycle reports directly to the founder:

- what changed and in which files;
- which gates passed, failed, or could not run;
- active workers and their exact scopes;
- blockers requiring network, credentials, or a founder decision;
- whether another cycle was armed.

## Safety boundaries

Workers may push normal branches/PRs when the repo protocol allows it. The
captain must pause for releases, public-source changes, force-pushes, deploys,
payment rails, and SII credentials. A runner may wake the captain, but it may
not silently perform those actions or claim 24/7 coverage without a live agent
session and observable heartbeat.
