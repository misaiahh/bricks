---
title: "Brick-Making Monorepo Setup — Design"
date: 2026-06-03T12:45:43+0800
author: Micky Hallabrin
status: ready
source: ".rpiv/artifacts/solutions/2026-06-03_12-45-43_brick-making-monorepo-setup.md"
---

# Brick-Making Monorepo Setup — Design

## Overview

Design for setting up a single git repository containing independent frontend applications (vanilla TypeScript + Vite) and Rust backend services for a brick-making business. Projects are architecturally independent — no shared code, no cross-project build dependencies. Linking is through AGENTS.md (AI agent guidance), Docker Compose (service orchestration), and documentation.

**Source:** Solutions artifact at `.rpiv/artifacts/solutions/2026-06-03_12-45-43_brick-making-monorepo-setup.md`
**Selected approach:** Single repo, no monorepo tooling. Each project is self-contained. Root AGENTS.md acts as AI agent router. Docker Compose orchestrates services.

## Summary

A clean, self-contained monorepo where each application (orders frontend, admin frontend, Rust backend services) lives in its own directory with its own dependencies, build tooling, and configuration. The root provides AI agent guidance via AGENTS.md, service orchestration via Docker Compose, and project overview via documentation. No shared configs, no build orchestration tools, no workspace managers.

## Architecture

### Directory Structure

```
bricks/
├── AGENTS.md                    # Root AI agent router
├── README.md                    # Project overview
├── ARCHITECTURE.md              # Technical architecture docs
├── docker-compose.yml           # Service orchestration
├── .gitignore
├── .editorconfig
├── apps/
│   ├── orders/                  # Public order creation frontend
│   │   ├── src/
│   │   │   ├── main.ts
│   │   │   ├── App.ts
│   │   │   ├── components/
│   │   │   ├── styles/
│   │   │   └── utils/
│   │   ├── public/
│   │   ├── index.html
│   │   ├── vite.config.ts
│   │   ├── vitest.config.ts
│   │   ├── eslint.config.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   └── admin/                   # Brick management admin frontend
│       ├── src/
│       │   ├── main.ts
│       │   ├── App.ts
│       │   ├── components/
│       │   ├── styles/
│       │   └── utils/
│       ├── public/
│       ├── index.html
│       ├── vite.config.ts
│       ├── vitest.config.ts
│       ├── eslint.config.js
│       ├── package.json
│       └── tsconfig.json
└── services/
    └── api/                     # Rust backend service
        ├── src/
        │   ├── main.rs
        │   └── handlers/
        ├── Cargo.toml
        └── .dockerignore
```

### AGENTS.md Structure

Root AGENTS.md acts as a router with progressive disclosure. It lists all subdirectories, their purpose, and where to find detailed guidance. Each subdirectory gets its own AGENTS.md (or .claude/CLAUDE.md) for context-specific guidance.

### Frontend App Structure (Shared Pattern)

Both `apps/orders` and `apps/admin` follow the same structure:
- **Vanilla TypeScript** — no framework (no React/Vue/Svelte)
- **Vite** — development server and bundler
- **Vitest** — unit and integration testing
- **Playwright** — end-to-end testing
- **ESLint** — linting
- **Prettier** — code formatting
- **No shared configs** — each app has its own eslint.config.js, vitest.config.ts, etc.

### Rust Backend Structure

- **Cargo workspace** at `services/` level
- Each Rust service has its own `Cargo.toml` and `src/`
- Docker-optimized build with `.dockerignore`
- No `package.json` sidecars (no monorepo tooling)

### Docker Compose

Root `docker-compose.yml` defines all services:
- `orders` — frontend app (served via Nginx or Vite dev server)
- `admin` — frontend app (served via Nginx or Vite dev server)
- `api` — Rust backend service
- Optional: `postgres`, `redis` if needed

### Documentation

- `README.md` — high-level project overview, quick start, links to sub-projects
- `ARCHITECTURE.md` — technical decisions, directory structure, tech stack rationale
- Per-app README.md files for project-specific docs

## File Map

| File | Purpose |
|------|---------|
| `AGENTS.md` | AI agent router — lists all subdirectories, their purpose, and where to find detailed guidance |
| `README.md` | Project overview, quick start, links to sub-projects |
| `ARCHITECTURE.md` | Technical architecture decisions and rationale |
| `docker-compose.yml` | Service orchestration for all apps and services |
| `.gitignore` | Standard ignores (node_modules, target, dist, .env) |
| `.editorconfig` | Editor consistency (indentation, trailing whitespace, encoding) |
| `apps/orders/package.json` | Orders app dependencies and scripts |
| `apps/orders/vite.config.ts` | Orders app Vite configuration |
| `apps/orders/tsconfig.json` | Orders app TypeScript configuration |
| `apps/orders/eslint.config.js` | Orders app ESLint configuration |
| `apps/orders/vitest.config.ts` | Orders app Vitest configuration |
| `apps/orders/src/main.ts` | Orders app entry point |
| `apps/orders/src/App.ts` | Orders app root component |
| `apps/admin/package.json` | Admin app dependencies and scripts |
| `apps/admin/vite.config.ts` | Admin app Vite configuration |
| `apps/admin/tsconfig.json` | Admin app TypeScript configuration |
| `apps/admin/eslint.config.js` | Admin app ESLint configuration |
| `apps/admin/vitest.config.ts` | Admin app Vitest configuration |
| `apps/admin/src/main.ts` | Admin app entry point |
| `apps/admin/src/App.ts` | Admin app root component |
| `services/api/Cargo.toml` | Rust backend crate definition |
| `services/api/src/main.rs` | Rust backend entry point |
| `services/api/.dockerignore` | Docker build ignores for Rust service |

## Slices

### Slice 1: Root Scaffolding and AGENTS.md

**What it delivers:** Foundation of the monorepo — root AGENTS.md, README.md, ARCHITECTURE.md, docker-compose.yml, .gitignore, .editorconfig. This slice establishes the repo structure and AI agent routing before any application code is added.

**Files:**
- `AGENTS.md`
- `README.md`
- `ARCHITECTURE.md`
- `docker-compose.yml`
- `.gitignore`
- `.editorconfig`

#### Automated Verification:
- [ ] AGENTS.md exists at repo root and lists all subdirectories
- [ ] README.md exists with project overview and quick start
- [ ] ARCHITECTURE.md exists with technical decisions
- [ ] docker-compose.yml exists and is valid YAML
- [ ] .gitignore covers node_modules, target, dist, .env
- [ ] .editorconfig exists with consistent settings

#### Manual Verification:
- [ ] AGENTS.md correctly routes AI agents to subdirectory-specific guidance
- [ ] docker-compose.yml references all expected services
- [ ] README.md provides clear entry point for new developers

---

### Slice 2: Orders App Scaffolding

**What it delivers:** A fully functional vanilla TypeScript + Vite project for the public order creation frontend. Includes build tooling (Vite, Vitest, ESLint, Prettier), project structure, entry points, and a basic working app that renders to the DOM. No business logic — just the scaffolding that proves the stack works.

**Files:**
- `apps/orders/package.json`
- `apps/orders/vite.config.ts`
- `apps/orders/tsconfig.json`
- `apps/orders/eslint.config.js`
- `apps/orders/vitest.config.ts`
- `apps/orders/index.html`
- `apps/orders/src/main.ts`
- `apps/orders/src/App.ts`

#### Automated Verification:
- [ ] `npm run dev` starts without errors
- [ ] `npm run build` produces a dist/ directory
- [ ] `npm run lint` passes with no errors
- [ ] `npm test` runs without errors (at least one passing test)

#### Manual Verification:
- [ ] Orders app loads in browser and renders content
- [ ] HMR works during development
- [ ] Build output is clean (no warnings)

---

### Slice 3: Admin App Scaffolding

**What it delivers:** A fully functional vanilla TypeScript + Vite project for the brick management admin frontend. Same tech stack as orders app but independent configuration. Includes build tooling, project structure, entry points, and a basic working app. No business logic — just the scaffolding that proves the stack works.

**Files:**
- `apps/admin/package.json`
- `apps/admin/vite.config.ts`
- `apps/admin/tsconfig.json`
- `apps/admin/eslint.config.js`
- `apps/admin/vitest.config.ts`
- `apps/admin/index.html`
- `apps/admin/src/main.ts`
- `apps/admin/src/App.ts`

#### Automated Verification:
- [ ] `npm run dev` starts without errors
- [ ] `npm run build` produces a dist/ directory
- [ ] `npm run lint` passes with no errors
- [ ] `npm test` runs without errors (at least one passing test)

#### Manual Verification:
- [ ] Admin app loads in browser and renders content
- [ ] HMR works during development
- [ ] Build output is clean (no warnings)

---

### Slice 4: Rust Backend Scaffolding

**What it delivers:** A Cargo workspace for the Rust backend services. Includes a basic axum-based HTTP service with a health check endpoint, Docker-optimized build configuration, and .dockerignore. No business logic — just the scaffolding that proves the Rust stack works and can be built and containerized.

**Files:**
- `services/api/Cargo.toml`
- `services/api/src/main.rs`
- `services/api/.dockerignore`

#### Automated Verification:
- [ ] `cargo build` compiles without errors
- [ ] `cargo test` runs without errors
- [ ] Docker image builds: `docker build -f services/api/Dockerfile services/api/`

#### Manual Verification:
- [ ] Rust service starts and responds to health check endpoint
- [ ] Docker image runs: `docker run --rm -p 8080:8080 <image>`

---

## Ordering Constraints

- **Slice 1** must run first — establishes repo structure, AGENTS.md, and docker-compose.yml that other slices reference
- **Slice 2** and **Slice 3** are independent of each other (can run in parallel after Slice 1)
- **Slice 4** is independent of Slices 2 and 3 (can run in parallel after Slice 1)
- All slices are independent of each other except for the root scaffolding in Slice 1

## Verification Notes

1. **AGENTS.md routing:** The root AGENTS.md must correctly list all subdirectories and their purposes. AI agents should be able to navigate from root to subdirectory guidance without confusion.
2. **Frontend stack parity:** Both orders and admin apps should have equivalent tooling configurations (same ESLint rules, same Prettier config, same Vitest setup) even though they're independent. This ensures consistent developer experience.
3. **Docker Compose validation:** The docker-compose.yml must be able to start all services (or at least the ones that are available) without errors. Non-existent services should be marked as `enabled: false` or use conditional service definitions.
4. **No shared configs:** Deliberately avoid shared ESLint/Prettier/tsconfig packages. Each app maintains its own config. If configs need to diverge later, they can — no workspace coupling.
5. **Rust Docker build:** The Rust service must have a multi-stage Dockerfile that produces a minimal production image. The .dockerignore should exclude target/ and build artifacts.

## Performance Considerations

1. **Frontend build times:** Vanilla TS + Vite should have fast build times (< 5s for dev, < 10s for production build). No shared config compilation overhead.
2. **Rust build times:** Rust builds are inherently slower. Multi-stage Docker builds should use cached layers. Consider `cargo-chef` for dependency caching if builds become a bottleneck.
3. **Docker Compose startup:** Services should have health checks to avoid race conditions during startup. The API service should start before frontend apps that depend on it.
4. **AI agent context:** AGENTS.md should be concise (under 200 lines at root) to avoid overwhelming AI agent context windows. Nested AGENTS.md files should be even shorter.

## Scope

### Building
- Root AGENTS.md file for AI agent routing
- Root documentation (README.md, ARCHITECTURE.md)
- Root docker-compose.yml for service orchestration
- Root .gitignore and .editorconfig
- `apps/orders/` — vanilla TS + Vite + Vitest + Playwright + ESLint + Prettier scaffolding
- `apps/admin/` — vanilla TS + Vite + Vitest + Playwright + ESLint + Prettier scaffolding
- `services/api/` — Rust backend scaffolding with Cargo and Docker

### NOT Building
- Actual business logic for orders or brick management
- Rust backend implementation beyond health check
- CI/CD pipeline configuration
- Database schema design
- Authentication/authorization
- API contract definitions (OpenAPI/Swagger)
- Shared configuration packages (ESLint, Prettier, tsconfig)
- Playwright E2E test implementation (config only)
- Dockerfile for frontend apps (Nginx serving static files)

## Developer Context

- Project is greenfield — no existing codebase
- Tech stack decision: vanilla TypeScript (no framework) for frontends, Rust for backends
- Monorepo tooling decision: none — projects are independent
- Linking mechanism: AGENTS.md, Docker Compose, documentation
- Rust web framework: axum (recommended for new projects, good async support, part of Tower ecosystem)
