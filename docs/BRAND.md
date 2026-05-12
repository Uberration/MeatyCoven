# OpenCoven Brand System

The canonical OpenCoven brand system lives in [`../DESIGN.md`](../DESIGN.md). Implementation assets live in [`../brand`](../brand).

## Core rule

OpenCoven should feel like **collective intelligence + controlled power**: arcane but precise, technical not gimmicky, powerful not loud, minimal but symbolic.

## Positioning

OpenCoven is an open ecosystem for persistent AI familiars: named agents with memory, tools, identity, roles, and continuity.

Most AI today feels temporary. You open a chat, explain your context, get a response, and start over. OpenCoven is built around a different future: AI that can **stay**.

OpenCoven gives builders a way to create durable AI systems that remember what matters, understand their purpose, use tools, collaborate with other agents, and remain understandable over time. Each familiar can have a name, a voice, a memory, a toolset, a role, and a place in a larger workflow.

The philosophy is simple: AI should be powerful without becoming opaque, personal without pretending to be human, and extensible without collapsing into chaos. OpenCoven brings structure to the magic through memory, identity, orchestration, local execution, tool access, and multi-agent collaboration.

Use this as the high-level brand promise:

> OpenCoven turns AI from a blank chatbox into a living workspace of agents that remember, coordinate, and belong to you.

## Brand asset pack

| Asset | Purpose |
| --- | --- |
| [`brand/logo/opencoven-logo.svg`](../brand/logo/opencoven-logo.svg) | Full-gradient primary logo for hero and social use |
| [`brand/logo/opencoven-mark.svg`](../brand/logo/opencoven-mark.svg) | Mark-only vector |
| [`brand/logo/opencoven-white.svg`](../brand/logo/opencoven-white.svg) | Solid white logo for small dark surfaces |
| [`brand/logo/opencoven-black.svg`](../brand/logo/opencoven-black.svg) | Solid black logo for light surfaces |
| [`brand/logo/opencoven-monoline.svg`](../brand/logo/opencoven-monoline.svg) | Technical diagrams and docs |
| [`brand/ui/color-tokens.css`](../brand/ui/color-tokens.css) | Canonical color tokens |
| [`brand/ui/typography.css`](../brand/ui/typography.css) | Canonical font stacks and tracking |
| [`brand/social/opencoven-og.png`](../brand/social/opencoven-og.png) | Social preview / OG image |
| [`brand/docs/BRAND-USAGE.md`](../brand/docs/BRAND-USAGE.md) | Contributor-facing usage checklist |

## Legacy raster icon pack

The existing raster icon pack remains available in [`assets/opencoven`](../assets/opencoven) for package README compatibility and platform slots. Treat `brand/logo` as canonical for new vector work.

## Package copies

The npm package READMEs use package-local copies of `opencoven.svg` so package previews do not depend on files outside the package tarball:

- [`packages/cli/assets/opencoven.svg`](../packages/cli/assets/opencoven.svg)
- [`packages/openclaw-coven/assets/opencoven.svg`](../packages/openclaw-coven/assets/opencoven.svg)

Keep those copies in sync with [`assets/opencoven/opencoven.svg`](../assets/opencoven/opencoven.svg).
