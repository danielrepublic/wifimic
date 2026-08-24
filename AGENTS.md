## Agent skills

### Issue tracker

GitHub Issues via the `gh` CLI (repo not yet initialized — will apply once a GitHub remote is added). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context — `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Release process

A version is not "done" until it has been pushed, tagged, and actually deployed and verified on the real two-machine environment (Linux `192.168.0.210` / Windows `192.168.0.200`) — a green CI build alone is not sufficient. Mandatory per-version checklist: see `docs/release-process.md`.
