---
title: "Brick-Making Monorepo Setup"
date: 2026-06-03T12:45:43+0800
author: Micky Hallabrin
status: ready
confidence: high
---

# Brick-Making Monorepo Setup — Solutions Analysis

## Overview

Setting up a single git repository containing independent frontend applications (vanilla TypeScript + Vite) and Rust backend services for a brick-making business. Projects are architecturally independent — no shared code, no cross-project build dependencies. Linking is through AGENTS.md (AI agent guidance), Docker Compose (service orchestration), and documentation.

## Existing Implementation

No existing codebase — this is a greenfield project. The project directory at `/Users/mickers/Projects/bricks` is empty.

## Solution Options

### Option 1: Single Repo, No Monorepo Tooling
**How it works:**
One git repo with each project as an independent subdirectory. Root AGENTS.md acts as an AI agent router. Docker Compose at root orchestrates services. Each project manages its own dependencies, build tooling, and configuration independently. No pnpm workspaces, no Turborepo, no Nx.

**Pros:**
- Minimal complexity — each project is self-contained
- No shared config coupling (a config change in one app doesn't affect another)
- Each frontend can use different Vite plugins or versions if needed later
- Rust backend uses Cargo independently, no package.json sidecar overhead
- Docker Compose is the natural orchestration layer
- AGENTS.md at root provides AI agent context without build-tooling complexity

**Cons:**
- No unified `pnpm install` across all projects (each project has its own `npm install` or `pnpm install`)
- No shared build caching across projects (not needed since projects are independent)
- No cross-project dependency graph (not needed since projects are independent)

**Complexity:** Low (~1 day to set up)
- Files to create: ~15 (root config + per-project scaffolding)
- Files to modify: 0
- Risk level: Low

### Option 2: Single Repo with pnpm Workspaces (No Turborepo)
**How it works:**
One git repo with pnpm workspaces for unified dependency management. Each project is a workspace member. Root `pnpm install` installs all dependencies. No build orchestration (no Turborepo/Nx).

**Pros:**
- Single `pnpm install` across all projects
- Faster installs than individual installs (hardlinking, shared node_modules)
- Still simple — no build orchestration complexity

**Cons:**
- pnpm workspaces require all workspace members to have `package.json` (including Rust projects — sidecar overhead)
- Shared `node_modules` hoisting can cause subtle issues with native modules (Rust binaries)
- Adds a dependency management layer that provides no benefit for independent projects
- If one project needs a different Node version or pnpm version, workspaces complicate this

**Complexity:** Low-Medium (~1-2 days to set up and debug)
- Files to create: ~18 (workspace config + per-project scaffolding)
- Files to modify: 0
- Risk level: Medium (native module conflicts)

### Option 3: Separate Repos with a Parent README
**How it works:**
Each project (orders app, admin app, each Rust service) is its own git repository. A parent directory or GitHub organization contains a README linking to all repos. AGENTS.md lives in each repo individually.

**Pros:**
- Maximum isolation — no accidental coupling
- Each project can have its own release cycle, CI/CD, and team ownership
- No cross-contamination of dependencies

**Cons:**
- Harder for AI agents to get full project context (need to know which repos to read)
- Docker Compose requires references to external repos or submodule setup
- Documentation is fragmented across repos
- User explicitly wants everything in one place

**Complexity:** Medium (~2-3 days to set up multiple repos)
- Files to create: ~15 per repo
- Files to modify: 0
- Risk level: Low

## Comparison

| Criteria | Option 1: No Tooling | Option 2: pnpm Workspaces | Option 3: Separate Repos |
|----------|---------------------|---------------------------|--------------------------|
| Complexity | Low | Low-Medium | Medium |
| Codebase Fit | High — matches independence | Medium — adds coupling | High — maximum isolation |
| Risk | Low | Medium (native modules) | Low |
| AI Agent Context | Excellent — single repo | Good — single repo | Poor — fragmented |
| Docker Compose | Natural fit | Natural fit | Requires submodules |
| Setup Time | ~1 day | ~1-2 days | ~2-3 days |

## Recommendation

**Selected:** Option 1 — Single Repo, No Monorepo Tooling

**Rationale:**
- Projects are architecturally independent with no shared code or build dependencies
- The only linking mechanisms are AGENTS.md, Docker Compose, and documentation — none of which require monorepo tooling
- No monorepo tooling means less complexity, fewer failure modes, and faster onboarding for AI agents
- Docker Compose is the natural orchestration layer for independent services
- Each project can evolve independently (different Vite versions, different Rust editions) without workspace constraints

**Why not alternatives:**
- Option 2 (pnpm workspaces): Adds dependency management overhead with no benefit. The sidecar `package.json` requirement for Rust projects is unnecessary complexity. Native module conflicts with Rust binaries are a real risk.
- Option 3 (separate repos): Fragments AI agent context and complicates Docker Compose setup. User explicitly wants everything in one place.

**Trade-offs:**
- Accepting per-project dependency installs for simpler project isolation and faster AI agent onboarding

**Implementation approach:**
1. Create root-level AGENTS.md as the AI agent router
2. Scaffold `apps/orders/` — vanilla TS + Vite + Vitest + Playwright + ESLint + Prettier
3. Scaffold `apps/admin/` — vanilla TS + Vite + Vitest + Playwright + ESLint + Prettier
4. Scaffold `services/` — Rust backend services (Cargo workspace)
5. Add root `docker-compose.yml` for service orchestration
6. Add root documentation (README.md, ARCHITECTURE.md)

**Integration points:**
- Root AGENTS.md references all subdirectories
- Root docker-compose.yml references all services
- Root README.md provides overview and links

**Patterns to follow:**
- AGENTS.md root + nested pattern (root as router, nested files per directory)
- Per-project self-containment (each project has its own package.json, configs, tests)
- Docker Compose for inter-service communication

**Risks:**
- None significant — greenfield project with clear boundaries

## Scope Boundaries

**Building:**
- Root AGENTS.md file for AI agent guidance
- Root README.md and ARCHITECTURE.md documentation
- Root docker-compose.yml for service orchestration
- `apps/orders/` — public-facing order creation frontend (vanilla TS, Vite, Vitest, Playwright, ESLint, Prettier)
- `apps/admin/` — brick management admin frontend (vanilla TS, Vite, Vitest, Playwright, ESLint, Prettier)
- `services/` — Rust backend service scaffolding (Cargo workspace)
- `.gitignore`, `.editorconfig`, root-level config files

**NOT building:**
- Actual business logic for orders or brick management
- Rust backend implementation details
- CI/CD pipeline configuration
- Database schema design
- Authentication/authorization
- API contract definitions (OpenAPI/Swagger)

## Testing Strategy

**Verification steps:**
- Each frontend app passes its own `npm test` (Vitest)
- Each frontend app passes its own `npm run lint` (ESLint)
- Docker Compose starts all services: `docker compose up --build`
- AGENTS.md is readable and provides correct routing guidance

## Precedents & Lessons

- [Turborepo multi-language guide](https://turborepo.dev/docs/guides/multi-language) — confirms that package.json sidecars for Rust work but add unnecessary complexity when projects are independent
- [agents.md spec](https://github.com/agentsmd/agents.md) — root + nested AGENTS.md pattern for monorepo AI routing
- [Datadog steering guide](https://dev.to/datadog-frontend-dev/steering-ai-agents-in-monorepos-with-agentsmd-13g0) — confirms root AGENTS.md as router with progressive disclosure
- [Nhost pnpm + Turborepo guide](https://nhost.io/blog/how-we-configured-pnpm-and-turborepo-for-monorepo) — demonstrates that monorepo tooling is valuable when projects share code/config, not when they're independent

## Open Questions

1. **Rust web framework**: Which Rust web framework for the backend? (axum, actix-web, warp?)
2. **Database**: What database will Rust services use? (PostgreSQL, SQLite, etc.)
3. **API style**: REST, GraphQL, or gRPC for frontend-to-backend communication?
4. **Deployment target**: Where will these services be deployed? (Docker-only, Kubernetes, serverless?)
5. **Authentication**: Will the admin app require authentication? What about the public orders app?
