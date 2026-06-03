---
date: "2026-06-03T06:07:48+00:00"
author: Micky Hallabrin
commit: 2108da6
branch: main
repository: bricks
topic: "Rust microservices architecture comparison"
confidence: high
complexity: medium
status: ready
tags: [solutions, rust, microservices, architecture]
last_updated: "2026-06-03T06:07:48+00:00"
last_updated_by: Micky Hallabrin
---

# Solution Analysis: Rust Microservices Architecture for Bricks

**Date**: 2026-06-03 06:07:48 UTC
**Author**: Micky Hallabrin
**Commit**: 2108da6
**Branch**: main
**Repository**: bricks

## Research Question

Compare three Rust microservices architecture approaches for a brick-making business monorepo, starting as a modular monolith and extracting services over time.

## Summary

**Problem**: Replace the single `services/api/` axum binary with a microservices architecture that can grow gradually from a modular monolith into independently deployable services.

**Recommended**: Option A — Workspace with separate crates. Each domain gets its own crate from day one, mounted in-process via a gateway. Extract to separate containers later when deployment independence is needed.

**Effort**: Medium (~10-15 days for Phase 0)

**Confidence**: High — this is the standard Rust workspace pattern with well-documented tooling and community precedent.

## Problem Statement

**Requirements:**
- Replace the current single `services/api/` binary with a microservices architecture
- Two frontend apps (`apps/orders`, `apps/admin`) that consume the backend
- Tech stack: axum + reqwest (REST), sqlx + SQLite, raw SQL, tokio, tracing
- Gradual split: start modular, extract services over time
- Small team (1-2 developers)

**Constraints:**
- SQLite (not PostgreSQL/MySQL) — shared-file database limits true concurrent write scaling
- No monorepo tooling currently (no Turborepo, Nx)
- Docker Compose for orchestration (not Kubernetes)
- Single codebase monorepo structure

**Success criteria:**
- Each domain (orders, users, employees, equipment, materials, notifications) has clear ownership boundaries
- Can deploy domains independently when needed
- Shared infrastructure (tracing, error handling, DB patterns) is DRY
- Frontend apps continue working with minimal changes

## Current State

**Existing implementation:**
- `services/api/Cargo.toml` — single crate `bricks-api` with axum 0.8, tokio, serde, tower, tracing
- `services/api/src/main.rs` — single binary with health endpoint, graceful shutdown, TraceLayer
- `docker-compose.yml` — 3 services (orders, admin, api), api has healthcheck on port 8080
- No database layer yet (no sqlx, no migrations, no models)
- No shared libraries between services
- No workspace configuration

**Relevant patterns:**
- `docker-compose.yml` — Docker Compose orchestration with healthchecks
- `services/api/src/main.rs` — axum Router pattern with `with_state()` and `TraceLayer`
- `AGENTS.md` — AI agent routing guide at repo root
- `ARCHITECTURE.md` — documents "no shared configs" and "no build orchestration" decisions

**Integration points:**
- `docker-compose.yml` — currently defines `api` service; will need to be updated for multiple services
- `apps/orders/src/App.ts` — consumes backend API (endpoint paths TBD)
- `apps/admin/src/App.ts` — consumes backend API (endpoint paths TBD)

## Solution Options

### Option A: Workspace with Separate Crates (Monorepo Services)

**How it works:**

A Cargo workspace under `services/` with separate crates for each domain. Each crate is both a `bin` and `lib` target. In Phase 0, a `gateway/` crate mounts all domain routers in-process. Shared infrastructure (`shared/`) provides tracing config, error types, DB connection pool patterns, and common middleware.

```
services/
├── Cargo.toml           ← workspace root
├── shared/              ← config, tracing, error, DB patterns
├── gateway/             ← routing layer, mounts all in-process
├── orders/              ← orders domain (bin + lib)
├── users/               ← auth domain (bin + lib)
├── employees/           ← employee domain (bin + lib)
├── equipment/           ← equipment domain (bin + lib)
├── materials/           ← materials domain (bin + lib)
└── notifications/       ← notifications domain (bin + lib)
```

**Pros:**
- Each domain crate is independently testable from day one (unit tests, integration tests)
- Clear ownership boundaries — each domain has its own module tree
- Gateway mounting is trivial with axum's `Router::merge()` or `nest()`
- Extracting a service to its own container is mechanical: copy the crate, add its own Dockerfile, update compose
- Shared crate prevents DRY violations in tracing, error handling, DB patterns
- Cargo workspace gives parallel compilation and unified `cargo test`

**Cons:**
- More Cargo.toml files to maintain (7-8 crates)
- In-process communication means no network-level fault isolation in Phase 0
- SQLite shared-file locking becomes a bottleneck if multiple domain crates write concurrently
- Gateway becomes a single point of failure for all domains

**Complexity:** Medium (~10-15 days for Phase 0)
- Files to create: ~15-20 (~800-1200 lines)
- Files to modify: 3 (docker-compose.yml, AGENTS.md, ARCHITECTURE.md)
- Risk level: Low — well-understood pattern, each crate is independently buildable

### Option B: Monolith That Grows Into Services

**How it works:**

A single `services/api/` crate that grows with domain modules. Domains are organized as submodules (`routes/orders.rs`, `handlers/orders.rs`, `models/order.rs`, etc.). One binary, one database, one Docker container. When a domain needs independent scaling, copy its modules into a new crate, set up inter-service communication, and add a new Docker container.

```
services/
├── Cargo.toml           ← single crate
├── src/
│   ├── main.rs
│   ├── routes/
│   │   ├── orders.rs
│   │   ├── users.rs
│   │   ├── employees.rs
│   │   └── materials.rs
│   ├── handlers/
│   ├── models/
│   └── db/
└── migrations/
```

**Pros:**
- Minimal initial complexity — one Cargo.toml, one binary, one Dockerfile
- Fast iteration — no cross-crate import overhead, no workspace configuration
- Easiest to get started — just add modules as features grow
- Single deployment unit means simple Docker Compose and CI
- SQLite shared-file access is trivial with one process

**Cons:**
- No natural ownership boundaries — modules in one file can freely import everything
- Testing gets harder as the binary grows — integration tests need to spin up the full router
- Extracting a domain later is a manual copy-paste operation with no automation
- Cargo compilation times grow linearly with total code
- Hard to assign domain ownership when everything is in one crate
- `main.rs` becomes a large dispatcher — harder to navigate than separate crates

**Complexity:** Low (~5-7 days for Phase 0)
- Files to create: ~8-10 (~300-500 lines)
- Files to modify: 2 (services/api/src/main.rs, docker-compose.yml)
- Risk level: Medium — works fine initially but creates technical debt that compounds during extraction

### Option C: Gateway + Service Registry Pattern

**How it works:**

Same crate structure as Option A (separate workspace crates), but each service runs as its own process from day one. A `gateway/` service handles service discovery (Eureka-style registry), health checking, and request routing between services. Services communicate via HTTP/REST using reqwest with retry and circuit breaker patterns.

```
services/
├── Cargo.toml
├── shared/
├── gateway/             ← service registry + health checking + routing
├── orders/              ← runs as separate process
├── users/               ← runs as separate process
├── employees/           ← runs as separate process
├── equipment/           ← runs as separate process
├── materials/           ← runs as separate process
└── notifications/       ← runs as separate process
```

**Pros:**
- True fault isolation — if orders crashes, materials keeps running
- Each service can scale independently
- Natural boundary for inter-service communication patterns (retry, circuit breaker, timeout)
- Service registry provides health monitoring and dynamic routing
- Closest to production-grade microservices architecture

**Cons:**
- Highest initial complexity — 7+ processes, service discovery, health checking
- Service registry adds operational burden (state management, failure modes)
- SQLite shared-file locking is problematic across processes — would need to migrate to WAL mode or switch databases
- Debugging is harder — requests span multiple processes, logs need correlation IDs
- Overkill for a small team with limited traffic
- Docker Compose becomes complex — 7+ service definitions, healthchecks, dependency chains
- The "registry" pattern is largely solved by Kubernetes or service meshes — adding it to Docker Compose is reinventing

**Complexity:** High (~20-30 days for Phase 0)
- Files to create: ~25-30 (~1500-2000 lines)
- Files to modify: 4+ (docker-compose.yml, AGENTS.md, ARCHITECTURE.md, CI config)
- Risk level: High — operational complexity exceeds team capacity, SQLite becomes a blocker

## Comparison

| Criteria | Option A: Workspace Crates | Option B: Monolith First | Option C: Registry Pattern |
|----------|---------------------------|-------------------------|---------------------------|
| **Implementation effort** | Medium (~10-15 days) | Low (~5-7 days) | High (~20-30 days) |
| **Gradual extraction** | Easy — crates already separate | Hard — manual copy-paste | N/A — already separate |
| **Docker/Compose story** | Simple — one container per service, add incrementally | Simplest — one container always | Complex — 7+ containers from day one |
| **Shared infrastructure** | `shared/` crate — DRY tracing, errors, DB patterns | Shared via `mod` — no extra crate needed | `shared/` crate + inter-service client libs |
| **Fault isolation** | None in Phase 0 (in-process) | None (single process) | Full (separate processes) |
| **Debugging** | Easy — single process, shared memory | Easiest — single process, single binary | Hard — cross-process, need correlation IDs |
| **SQLite compatibility** | Good in Phase 0 (single process) | Best — single writer | Problematic — concurrent writes need WAL or DB migration |
| **Team fit (1-2 devs)** | Good — clear ownership, manageable complexity | Good for very small teams | Poor — operational overhead exceeds team size |
| **CI complexity** | Moderate — workspace test, per-crate lint | Low — single crate | High — per-service CI, integration tests |
| **Long-term scalability** | High — extraction is mechanical | Medium — extraction creates debt | Highest — already distributed |

## Recommendation

**Selected: Option A — Workspace with Separate Crates**

**Rationale:**

1. **Optimal extraction path.** Each domain crate is independently buildable and testable from day one. When it's time to extract a service to its own container, the work is mechanical: add a Dockerfile, update `docker-compose.yml`, and point the gateway at the new port. With Option B, extraction is a risky manual copy-paste with no safety net.

2. **Clear ownership boundaries.** With 6+ domains, separate crates naturally enforce module boundaries. `orders/` cannot accidentally import `employees/` internals — it must go through the shared crate or public API. This prevents the "everything imports everything" problem that kills monoliths.

3. **Manageable for a small team.** Option A keeps Phase 0 as a single process (gateway mounts all crates in-process), so debugging, testing, and deployment are as simple as Option B. The complexity only increases when you extract services — and you control that timeline.

4. **Shared infrastructure is DRY.** The `shared/` crate provides common patterns (error types, tracing config, DB connection pool setup) that all domain crates import. This is the standard Rust workspace pattern and is well-supported by Cargo.

5. **SQLite works in Phase 0.** Running all crates in one process means SQLite shared-file access is fine. When you extract a service, you can either keep SQLite with WAL mode (read-heavy workloads) or migrate that service to its own database.

**Why not Option B:**
- Option B's simplicity is seductive but creates compounding technical debt. Extracting a domain from a monolith is a high-risk, manual operation with no automated safety net. With Option A, extraction is a low-risk, mechanical operation. For a team that plans to extract services, Option A is the safer choice despite slightly higher initial effort.

**Why not Option C:**
- Option C is over-engineered for a small team running on Docker Compose. Service discovery, health checking, and circuit breakers are valuable at scale, but they add operational complexity that a 1-2 person team cannot sustain. SQLite shared-file locking also becomes a hard blocker when services run as separate processes. Start with Option A and add service mesh/registry patterns only when you have evidence that in-process communication is a bottleneck.

**Trade-offs:**
- Accepting slightly higher initial setup (~10-15 days vs ~5-7 days) for Option B in exchange for painless extraction later
- Accepting single-process fault isolation in Phase 0 in exchange for simplicity, deferring fault isolation to Phase 1 when it's actually needed

**Implementation approach:**

**Phase 0 — Workspace + Gateway (Week 1-2)**
1. Create `services/Cargo.toml` workspace with `shared/`, `gateway/`, and empty domain crate stubs
2. Move existing health endpoint into `shared/` or `gateway/`
3. Implement domain routers as lib crates (no handlers yet, just module structure)
4. Gateway mounts all domain routers via `Router::nest()` or `Router::merge()`
5. Single Docker container running the gateway binary
6. Frontend apps unchanged — they still hit the same gateway endpoints

**Phase 1 — First Service Extraction (Week 3-4)**
1. Pick the most stable/least-changing domain (e.g., `materials` or `equipment`)
2. Add its Dockerfile, update `docker-compose.yml`
3. Gateway routes to the extracted service via HTTP (reqwest)
4. Add healthcheck and retry logic to gateway's inter-service calls

**Phase 2 — Gradual Extraction (Ongoing)**
1. Extract one domain per sprint based on deployment frequency needs
2. Add correlation IDs for cross-service tracing
3. Evaluate SQLite WAL mode vs per-service databases

**Integration points:**
- `docker-compose.yml` — Phase 0: single `api` container. Phase 1+: add extracted service containers, gateway updates to route via HTTP
- `apps/orders/src/App.ts` — unchanged in Phase 0, may need endpoint prefix changes if gateway restructures routes
- `apps/admin/src/App.ts` — unchanged in Phase 0, may need endpoint prefix changes if gateway restructures routes

**Patterns to follow:**
- `services/api/src/main.rs` — keep the graceful shutdown pattern; each extracted service reuses `shared/`'s shutdown utilities
- `services/api/src/main.rs` — keep the tracing-subscriber + env-filter pattern in `shared/`
- `docker-compose.yml` — keep healthcheck pattern; each service gets its own health endpoint

**Risks:**
- **SQLite concurrent writes**: Mitigation — Phase 0 runs in-process (no concurrency issue). Phase 1+ extraction: use WAL mode, or migrate write-heavy services to a separate database
- **Gateway becomes a bottleneck**: Mitigation — monitor gateway CPU/memory. If it becomes a bottleneck, extract it into its own process (still in-process routing via `Router::merge()` is fast enough for most workloads)
- **Domain coupling through shared crate**: Mitigation — keep `shared/` infrastructure-only (no domain logic). Each domain crate owns its own models and handlers

## Scope Boundaries
- **Building**: Workspace structure, gateway, shared crate, domain crate stubs, Phase 0 deployment
- **Not building**: Actual domain handlers/models (orders, users, etc.), database migrations, authentication, inter-service communication patterns (Phase 2+)

## Testing Strategy

**Unit tests:**
- Each domain crate's lib module: test domain logic in isolation (mock the shared crate's DB interface)
- Shared crate: test error conversion, tracing config, connection pool setup
- Gateway: test route mounting, request forwarding

**Integration tests:**
- Gateway mounts all domain routers and verifies each health endpoint returns 200
- Docker Compose integration: `docker compose up` starts all services, healthcheck passes

**Manual verification:**
- [ ] `cargo build --workspace` compiles all crates
- [ ] `cargo test --workspace` runs tests across all crates
- [ ] `docker compose up` starts the gateway, healthcheck passes
- [ ] `curl localhost:8080/health` returns `{"status":"ok","service":"bricks-gateway"}`
- [ ] Frontend apps can reach the gateway at the expected endpoints

## Open Questions

**Resolved during research:**
- SQLite compatibility across services — Answer: fine in Phase 0 (single process), requires WAL mode or separate databases in Phase 1+ (see `services/api/Cargo.toml` — no DB driver yet, so migration timing is flexible)
- Service discovery necessity — Answer: not needed until Phase 2+. Docker Compose static configuration is sufficient for Phase 0 and Phase 1 (see `docker-compose.yml` — already uses static port mapping)

**Requires user input:**
- Domain priority for extraction — Default assumption: extract `materials` or `equipment` first (least frequently changing), then `orders` (highest traffic), then `users` (needs auth), then `notifications` (async)
- Database strategy for extracted services — Default assumption: keep SQLite with WAL mode for read-heavy services, migrate write-heavy services to PostgreSQL

**Blockers:**
- None — all options are viable, recommendation is clear given the constraints

## References

- `services/api/Cargo.toml` — current single-crate configuration, base dependencies
- `services/api/src/main.rs` — current axum patterns (Router, TraceLayer, graceful shutdown)
- `docker-compose.yml` — current orchestration, healthcheck pattern
- `ARCHITECTURE.md` — current architecture decisions (no shared configs, no build orchestration)
- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) — Rust workspace documentation
- [axum routing](https://docs.rs/axum/latest/axum/struct.Router.html) — axum Router documentation for `merge()` and `nest()`
