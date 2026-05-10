# OpenCoven Documentation

This directory holds the product and architecture notes for Coven, the OpenCoven harness substrate.

## Start here

- [Getting started](GETTING-STARTED.md) — first install, first daemon, first session, and contributor checks.
- [Concepts](CONCEPTS.md) — core nouns: harness, project root, session, daemon, client, control plane, and rituals.
- [Glossary](GLOSSARY.md) — short definitions for recurring product and architecture terms.
- [Public roadmap](ROADMAP.md) — community-facing progress snapshot across Coven, comux, and integrations.
- [Product spec](PRODUCT-SPEC.md) — what Coven is and what belongs in MVP.
- [Architecture diagrams](ARCHITECTURE.md) — runtime topology, session lifecycle, and authority boundary diagrams.
- [Session lifecycle](SESSION-LIFECYCLE.md) — launch, attach/replay, archive, summon, sacrifice, orphan recovery, and events.
- [Local API](API.md) — versioned Unix-socket API contract for comux, OpenClaw, and other clients.
- [Operational model](OPERATIONAL-MODEL.md) — authority boundaries between Rust, comux, OpenClaw, and npm wrappers.
- [Safety model](SAFETY-MODEL.md) — local trust boundary, secret handling, socket posture, and automation approvals.
- [Client integration guide](CLIENT-INTEGRATION.md) — expectations for comux, OpenClaw, OpenMeow, desktop clients, and future control rooms.
- [Harness adapter guide](HARNESS-ADAPTERS.md) — current Codex/Claude adapter shape and the bar for future harness support.
- [Troubleshooting](TROUBLESHOOTING.md) — common setup, daemon, harness, session, API, and verification problems.
- [MVP plan](MVP-PLAN.md) — implementation plan and success criteria.
- [Future harnesses](FUTURE-HARNESSES.md) — adapter direction after Codex and Claude Code.
- [Brand assets](BRAND.md) — canonical logo, token, typography, and social asset pack.
- [Brand adherence checklist](BRANDING-ADHERENCE.md) — implementation progress, exceptions, and risks.
- [Documentation maintenance](DOCS-MAINTENANCE.md) — public-doc rules, canonical names, secret avoidance, and verification checks.

## OpenClaw rescue loop

Coven can help repair a local OpenClaw source checkout without relying on a healthy OpenClaw runtime:

```sh
coven patch openclaw
coven patch openclaw "fix Codex auth profile order after invalidated OAuth token"
coven patch openclaw --repo ~/Documents/GitHub/openclaw/openclaw --harness codex --dry-run
```

The guided flow detects the repo, asks what is broken, launches a supervised Codex or Claude Code session, runs verification, and reports changed files. Coven does not commit or push in v0.

## Documentation stance

Keep these docs aligned with VMUX-style clarity while staying specific to OpenCoven:

- Short public-facing README first.
- Concrete quick-start commands.
- Explicit local trust boundary.
- Clear relationship to comux and OpenClaw.
- Be precise about npm status: packages are published for early adopters, but Coven is still an early local-first MVP.

## Canonical language

- Ecosystem/org: **OpenCoven**
- Product/daemon/CLI: **Coven**
- Command: `coven`
- CLI package: `@opencoven/cli`
- OpenClaw plugin package: `@opencoven/coven`
- Community: `discord.gg/opencoven`
- X / Twitter: `@OpenCvn`
