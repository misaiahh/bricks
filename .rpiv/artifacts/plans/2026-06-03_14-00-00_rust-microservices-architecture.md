---
date: 2026-06-03T14:00:00+0800
author: Micky Hallabrin
commit: 2108da6
branch: main
repository: bricks
topic: "Rust Microservices Architecture — Phase 0 (Modular Monolith)"
tags: [plan, rust, microservices, workspace, axum, sqlite]
status: ready
parent: ".rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md"
last_updated: 2026-06-03T14:00:00+0800
last_updated_by: Micky Hallabrin
---

# Rust Microservices Architecture — Implementation Plan (Phase 0)

## Overview

Replace the single `services/api/` axum binary with a Cargo workspace containing a `shared/` infrastructure crate, a `gateway/` routing crate, and stub crates for 6 domains (orders, users, employees, equipment, materials, notifications). Phase 0 runs everything in-process via the gateway — a modular monolith. Each domain crate has both `bin` and `lib` targets so extraction to separate containers is mechanical in Phase 1+.

**Tech stack decisions:**
- **Inter-service communication:** REST (axum + reqwest) — uniform across all services
- **Database:** SQLite via sqlx (one DB per service, WAL mode for Phase 1+)
- **ORM:** Raw SQL (not SeaORM/Diesel) — simpler, compile-time checked via sqlx
- **Runtime:** tokio (shared across all services)
- **Observability:** tracing + tracing-subscriber (structured JSON logs)
- **Error handling:** thiserror

**Design source:** `.rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md`
**Solutions source:** `.rpiv/artifacts/solutions/rust-microservices-architecture.md`
**Selected approach:** Option A — Workspace with separate crates, modular monolith in Phase 0

## Desired End State

After Phase 0, the developer can:

1. **Build the entire workspace:** `cargo build --workspace && cargo test --workspace`
2. **Run the gateway locally:** `cargo run --package bricks-gateway` → listens on port 8080
3. **Hit the health endpoint:** `curl http://localhost:8080/health` → `{"status":"ok","service":"bricks-gateway"}`
4. **Build and run via Docker:** `docker compose up --build gateway`
5. **Add a handler to any domain:** The gateway automatically routes by path prefix (`/orders/*`, `/users/*`, etc.)

## What We're NOT Doing (Phase 0)

- Actual domain handlers, models, or business logic — all domain crates are stubs
- Database migrations or sqlx setup — no DB layer in Phase 0
- Authentication/authorization
- Inter-service HTTP communication (reqwest-based routing) — Phase 1+
- Individual Dockerfiles per domain — Phase 1+
- Frontend app changes — frontends are static placeholders
- CI pipeline changes

## Phase 1: Workspace Root + Shared Crate

### Overview
Create the Cargo workspace root at `services/Cargo.toml` with shared dependencies, and the `shared/` infrastructure crate providing error types, tracing config, graceful shutdown, and the `DomainRouter` trait.

### Changes Required:

#### 1. services/Cargo.toml — Workspace Root
**File:** `services/Cargo.toml` (NEW)
**Changes:** Workspace definition with 8 members and shared dependencies.

```toml
[workspace]
members = [
    "shared",
    "gateway",
    "orders",
    "users",
    "employees",
    "equipment",
    "materials",
    "notifications",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
axum = { version = "0.8", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1.0"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite"] }
reqwest = { version = "0.12", features = ["json"] }
uuid = { version = "1", features = ["v4", "serde"] }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
```

#### 2. services/shared/Cargo.toml — Shared Crate Manifest
**File:** `services/shared/Cargo.toml` (NEW)

```toml
[package]
name = "bricks-shared"
version.workspace = true
edition.workspace = true

[dependencies]
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
thiserror = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true, features = ["cors", "compression"] }
sqlx = { workspace = true, optional = true }
reqwest = { workspace = true, optional = true }
uuid = { workspace = true, optional = true }
```

#### 3. services/shared/src/lib.rs — Shared Infrastructure
**File:** `services/shared/src/lib.rs` (NEW)
**Changes:** Error types, tracing config, graceful shutdown, `ServerConfig`, `DomainRouter` trait, CORS layer.

```rust
use axum::{
    Router,
    http::Request,
    middleware::Next,
    response::Response,
};
use serde::Serialize;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};

// ─── Error Types ─────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum AppError {
    #[error("internal server error")]
    Internal,

    #[error("not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("conflict: {0}")]
    Conflict(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match &self {
            AppError::Internal => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::NotFound => (axum::http::StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (axum::http::StatusCode::CONFLICT, msg.clone()),
        };
        (status, body).into_response()
    }
}

// ─── Health Response ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: String,
}

// ─── Tracing Config ──────────────────────────────────────────────────────────

pub fn init_tracing(service_name: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    format!("{service_name}=debug,axum=info,tower_http=debug").into()
                }),
        )
        .init();
}

// ─── Graceful Shutdown ───────────────────────────────────────────────────────

pub async fn shutdown_handler() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = async { std::future::pending::<()>().await; };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received");
}

// ─── Server Config ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl ServerConfig {
    pub fn addr(&self) -> SocketAddr {
        let host = if self.host.is_empty() {
            "0.0.0.0".to_string()
        } else {
            self.host.clone()
        };
        format!("{}:{}", host, self.port)
            .parse()
            .expect("failed to parse socket address")
    }
}

// ─── Domain Router Trait ─────────────────────────────────────────────────────

/// Trait that domain crates implement to expose their router.
/// In Phase 0, the gateway calls this trait method to mount routers.
/// In Phase 1+, domains are standalone services and the trait is unused.
pub trait DomainRouter {
    fn router() -> Router;
}

// ─── CORS Layer ──────────────────────────────────────────────────────────────

pub fn cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::permissive()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response() {
        let resp = HealthResponse {
            status: "ok",
            service: "test".to_string(),
        };
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.service, "test");
    }

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig {
            host: String::new(),
            port: 8080,
        };
        let addr = config.addr();
        assert_eq!(addr.port(), 8080);
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --workspace` compiles the workspace root and shared crate
- [ ] `cargo test --workspace` runs tests in the shared crate (2 tests pass)
- [ ] `services/Cargo.toml` contains all 8 workspace members
- [ ] Shared crate exports: `AppError`, `HealthResponse`, `init_tracing()`, `shutdown_handler()`, `ServerConfig`, `DomainRouter` trait, `cors_layer()`

#### Manual Verification:
- [ ] Shared crate compiles independently: `cargo build -p bricks-shared`
- [ ] `init_tracing()` configures tracing-subscriber with env-filter
- [ ] `shutdown_handler()` handles both Ctrl+C and SIGTERM

---

## Phase 2: Gateway Crate

### Overview
Create the `gateway/` crate that imports all 6 domain crates (via their `lib` targets) and mounts their routers via `Router::nest()`. Also includes a Dockerfile for Phase 0 single-container deployment.

### Changes Required:

#### 1. services/gateway/Cargo.toml — Gateway Manifest
**File:** `services/gateway/Cargo.toml` (NEW)

```toml
[package]
name = "bricks-gateway"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-gateway"
path = "src/main.rs"

[dependencies]
bricks-shared = { path = "../shared" }
bricks-orders = { path = "../orders" }
bricks-users = { path = "../users" }
bricks-employees = { path = "../employees" }
bricks-equipment = { path = "../equipment" }
bricks-materials = { path = "../materials" }
bricks-notifications = { path = "../notifications" }
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

#### 2. services/gateway/src/main.rs — Gateway Binary
**File:** `services/gateway/src/main.rs` (NEW)

```rust
use axum::{
    routing::get,
    Router,
};
use bricks_shared::{
    cors_layer,
    init_tracing,
    shutdown_handler,
    HealthResponse,
    ServerConfig,
};

async fn health() -> axum::response::Json<HealthResponse> {
    axum::response::Json(HealthResponse {
        status: "ok",
        service: "bricks-gateway".to_string(),
    })
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_gateway");

    let app = Router::new()
        .route("/health", get(health))
        .nest("/orders", bricks_orders::router())
        .nest("/users", bricks_users::router())
        .nest("/employees", bricks_employees::router())
        .nest("/equipment", bricks_equipment::router())
        .nest("/materials", bricks_materials::router())
        .nest("/notifications", bricks_notifications::router())
        .layer(cors_layer())
        .layer(tower::ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let config = ServerConfig {
        host: String::new(),
        port: 8080,
    };
    let addr = config.addr();
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}
```

#### 3. services/gateway/Dockerfile — Multi-Stage Build
**File:** `services/gateway/Dockerfile` (NEW)

```dockerfile
FROM rust:slim AS builder

WORKDIR /app

# Copy workspace root and all crates
COPY Cargo.toml ./
COPY shared/ ./shared/
COPY gateway/ ./gateway/
COPY orders/ ./orders/
COPY users/ ./users/
COPY employees/ ./employees/
COPY equipment/ ./equipment/
COPY materials/ ./materials/
COPY notifications/ ./notifications/

# Build the gateway (which pulls in all workspace dependencies)
RUN cargo build --release --package bricks-gateway

# Runtime image
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/bricks-gateway .

EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=5s --retries=3 --start-period=30s \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["./bricks-gateway"]
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --package bricks-gateway` compiles the gateway
- [ ] `cargo test --workspace` passes (shared crate tests only)
- [ ] Gateway binary is produced at `target/release/bricks-gateway`
- [ ] Gateway imports all 6 domain crates from workspace

#### Manual Verification:
- [ ] Gateway mounts all 6 domain routers via `Router::nest()`
- [ ] Health endpoint at `GET /health` returns `{"status":"ok","service":"bricks-gateway"}`
- [ ] Graceful shutdown handles Ctrl+C and SIGTERM
- [ ] Dockerfile builds successfully: `docker build -f services/gateway/Dockerfile -t bricks-gateway:latest .`

---

## Phase 3: Domain Crate Stubs (All 6 Domains)

### Overview
Create stub crates for all 6 domains. Each crate has both `lib` and `bin` targets. The `lib` target exports an empty `router()` function. The `bin` target starts a standalone process with a health endpoint. Domain crates only depend on `bricks-shared` — no cross-domain dependencies.

### Changes Required:

For each domain (orders, users, employees, equipment, materials, notifications):

#### Cargo.toml

```toml
[package]
name = "bricks-<domain>"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-<domain>"
path = "src/main.rs"

[lib]
name = "bricks_<domain>"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
```

#### src/lib.rs — Domain Library

```rust
use axum::Router;

/// Returns an empty router for this domain.
/// Handlers will be added in future phases.
pub fn router() -> Router {
    Router::new()
}
```

#### src/main.rs — Domain Binary (Standalone)

```rust
use axum::{
    routing::get,
    Router,
};
use bricks_shared::{
    init_tracing,
    shutdown_handler,
    HealthResponse,
    ServerConfig,
};

async fn health() -> axum::response::Json<HealthResponse> {
    axum::response::Json(HealthResponse {
        status: "ok",
        service: "<domain>".to_string(),
    })
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_<domain>");

    let app = Router::new()
        .route("/health", get(health))
        .nest("/", bricks_<domain>::router());

    let config = ServerConfig {
        host: String::new(),
        port: 8081, // Each domain uses a different port when standalone
    };
    let addr = config.addr();
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --workspace` compiles all 8 crates (shared + gateway + 6 domains)
- [ ] `cargo test --workspace` runs tests in all crates
- [ ] Each domain crate has both `lib` and `bin` targets
- [ ] Each domain crate exports a `<domain>_router()` function
- [ ] Each domain crate only depends on `bricks-shared` (no cross-domain dependencies)

#### Manual Verification:
- [ ] Each domain crate compiles independently: `cargo build -p bricks-<domain>`
- [ ] Each domain `lib.rs` exports a `router()` function returning `axum::Router`
- [ ] Each domain `main.rs` starts a standalone process with health endpoint
- [ ] Domain crates have no cross-domain dependencies

---

## Phase 4: Docker Compose + Documentation Updates

### Overview
Remove the old `services/api/` directory, update `docker-compose.yml` to replace the `api` service with `gateway`, and update `ARCHITECTURE.md` and `AGENTS.md` to reflect the new workspace structure.

### Changes Required:

#### 1. Remove `services/api/`
**Action:** Delete the entire `services/api/` directory.

#### 2. docker-compose.yml — Update Service Names
**File:** `docker-compose.yml` (MODIFY)
**Changes:** Replace `api` service with `gateway`, update frontend `depends_on`.

```yaml
version: "3.9"

services:
  orders:
    image: bricks-orders:latest
    ports:
      - "3000:80"
    depends_on:
      gateway:
        condition: service_healthy
    restart: unless-stopped

  admin:
    image: bricks-admin:latest
    ports:
      - "3001:80"
    depends_on:
      gateway:
        condition: service_healthy
    restart: unless-stopped

  gateway:
    build:
      context: ./services
      dockerfile: gateway/Dockerfile
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 3
    restart: unless-stopped
```

#### 3. ARCHITECTURE.md — Update Directory Structure
**File:** `ARCHITECTURE.md` (MODIFY)
**Changes:** Update directory structure and tech stack documentation.

```markdown
# Architecture

## Directory Structure
```
bricks/
├── AGENTS.md
├── README.md
├── ARCHITECTURE.md
├── docker-compose.yml
├── .gitignore
├── .editorconfig
├── apps/
│   ├── orders/    # Public order creation frontend
│   └── admin/     # Brick management admin frontend
└── services/
    ├── Cargo.toml             # Workspace root
    ├── shared/                # Shared infrastructure (errors, tracing, config)
    ├── gateway/               # API gateway (routes to all domains)
    ├── orders/                # Orders domain (stub)
    ├── users/                 # Users domain (stub)
    ├── employees/             # Employees domain (stub)
    ├── equipment/             # Equipment domain (stub)
    ├── materials/             # Materials domain (stub)
    └── notifications/         # Notifications domain (stub)
```

## Tech Stack
- **Frontends:** Vanilla TypeScript, Vite (build/dev), Vitest (unit tests), Playwright (E2E), ESLint, Prettier
- **Backend:** Rust, axum (web framework), sqlx (async SQL), SQLite (database), Cargo (build tool)
- **Orchestration:** Docker Compose
- **No monorepo tooling:** Each project is self-contained with its own dependencies and configs
- **Communication:** REST (axum + reqwest) — all services use the same HTTP stack

## Design Decisions
- No shared configuration packages — each project maintains its own ESLint, Prettier, and TypeScript configs
- No build orchestration tools (Turborepo, Nx) — projects are independent
- AGENTS.md at root for AI agent routing with progressive disclosure
- Docker Compose for service orchestration
- **Microservices workspace:** Backend is a Cargo workspace with separate crates per domain, running as a modular monolith in Phase 0
- **Database-per-service:** Each domain owns its own SQLite database (WAL mode for Phase 1+ extraction)
- **Gradual split:** Start as single binary (gateway), extract domains to separate containers as traffic grows
```

#### 4. AGENTS.md — Update Backend Reference
**File:** `AGENTS.md` (MODIFY)
**Changes:** Update backend reference from `services/api/` to workspace structure.

```markdown
# Bricks Monorepo — AI Agent Guide

## Repo Structure
- `apps/orders/` — Public order creation frontend (vanilla TS, Vite, Vitest, Playwright)
- `apps/admin/` — Brick management admin frontend (vanilla TS, Vite, Vitest, Playwright)
- `services/` — Rust backend microservices workspace
  - `gateway/` — API gateway (routes to all domain services)
  - `shared/` — Shared infrastructure (errors, tracing, config)
  - `orders/` — Orders domain
  - `users/` — Users domain
  - `employees/` — Employees domain
  - `equipment/` — Equipment domain
  - `materials/` — Materials domain
  - `notifications/` — Notifications domain

## Commands
- `npm install` — Install dependencies for current directory
- `cargo build --workspace` — Build all Rust services
- `cargo run --package bricks-gateway` — Run the gateway locally
- `docker compose up` — Start all services
- `docker compose up --build gateway` — Start gateway service only

## Shared Rules
- Each project is self-contained — no shared configs between projects
- Frontend apps: vanilla TypeScript, no framework
- Backend services: Rust with Cargo workspace, axum for HTTP
- Run linting and tests before committing
- Use Docker Compose for service orchestration

## When to Read More
- For orders app details → read `apps/orders/README.md`
- For admin app details → read `apps/admin/README.md`
- For backend details → read `ARCHITECTURE.md`
- For architecture decisions → read `ARCHITECTURE.md`
```

### Success Criteria:

#### Automated Verification:
- [ ] `services/api/` directory is removed
- [ ] `docker compose config` validates the updated compose file
- [ ] `docker compose up --build gateway` starts the gateway service
- [ ] Gateway healthcheck passes: `curl -f http://localhost:8080/health`
- [ ] `ARCHITECTURE.md` reflects new workspace structure
- [ ] `AGENTS.md` references `services/` workspace instead of `services/api/`

#### Manual Verification:
- [ ] Frontend apps still depend on `gateway` (renamed from `api`) in `depends_on`
- [ ] Port 8080 is unchanged — frontend apps have no hardcoded backend URLs (static placeholders)
- [ ] Gateway runs and responds to health check

---

## Testing Strategy

### Automated:
- Phase 1: `cargo build --workspace`, `cargo test --workspace` (shared crate tests)
- Phase 2: `cargo build --package bricks-gateway`, Docker build
- Phase 3: `cargo build --workspace` (all 8 crates), `cargo test --workspace`
- Phase 4: `docker compose config`, `docker compose up --build gateway`, health check

### Manual Testing Steps:
1. Run `cargo run --package bricks-gateway` — should start on port 8080
2. Hit `curl http://localhost:8080/health` — should return `{"status":"ok","service":"bricks-gateway"}`
3. Hit `curl http://localhost:8080/orders` — should return 404 (no handlers yet)
4. Hit `curl http://localhost:8080/users` — should return 404 (no handlers yet)
5. Run `docker compose up --build gateway` — gateway should start and pass healthcheck
6. Verify `services/api/` is gone

## Performance Considerations

1. **In-process routing (Phase 0):** `Router::nest()` adds minimal overhead — just path prefix matching. No serialization/deserialization.
2. **Gateway as single point:** All domains run in the same process. Gateway CPU/memory is the bottleneck.
3. **Extraction overhead (Phase 1+):** When extracting a service, inter-service communication adds HTTP latency (reqwest round-trip). Typical overhead: 1-5ms per request within the same Docker network.
4. **SQLite read paths:** WAL mode enables concurrent reads. Write contention is the primary concern — design write-heavy domains to use their own database in Phase 1+.

## Migration Notes

- **`services/api/` removal:** The old `services/api/` directory is completely replaced. Its `Cargo.toml` and `src/main.rs` are not migrated — the workspace structure is a fresh start. This is safe because:
  - No database layer exists yet (no sqlx, no migrations, no models to migrate)
  - Only one endpoint exists (health), which is replicated in the gateway
  - No domain handlers exist yet
- **Docker image names:** `bricks-api` → `bricks-gateway`. Docker Compose service name changes from `api` to `gateway`. Frontend apps' `depends_on` updated accordingly.
- **No breaking changes for frontend:** Frontend apps are static placeholders. When they start calling the backend, they'll hit the gateway at the same port (8080) with the same health endpoint path (`/health`).
- **Rollback:** If Phase 0 fails, the old `services/api/` can be restored from git: `git checkout HEAD -- services/api/`. The workspace structure is additive — it doesn't modify existing files until Phase 4.

## Developer Context

- **Question:** Which domains should get stub crates in Phase 0?
  - **Answer:** All six domains (orders, users, employees, equipment, materials, notifications). Establishes ownership boundaries for every domain from day one.
- **Question:** Gateway routing: `nest()` vs `merge()`?
  - **Answer:** `nest()` for path-based domain routing (`/orders`, `/users`, etc.). Simpler, cleaner URL structure. `merge()` can be used later if flat routing is needed.
- **Question:** SQLite strategy for Phase 1+ extraction?
  - **Answer:** WAL mode (`PRAGMA journal_mode=WAL`) for read-heavy services. Separate databases for write-heavy services. Document in Phase 1 planning.
- **Question:** Frontend app changes needed?
  - **Answer:** None in Phase 0. Frontend apps are static placeholders with no API calls. When they start calling the backend, they'll hit the gateway at port 8080.

## Plan Review (Step 4)

_Independent post-finalization review by artifact-code-reviewer and artifact-coverage-reviewer subagents. Findings triaged at Step 5._

| source | plan-loc | codebase-loc | severity | dimension | finding | recommendation | resolution |
|--------|----------|--------------|----------|-----------|---------|----------------|------------|
| code | Phase 2 (gateway Dockerfile) | <n/a> | concern | actionability | Gateway Dockerfile copies all 8 crates into builder — includes domain crates that gateway doesn't need at compile time | Only copy `shared/`, `gateway/`, and domain crates that the gateway actually imports. Since gateway imports all 6 domain libs, this is correct. | verified: Gateway imports all 6 domain libs, so all must be in the build context. |
| code | Phase 3 (domain main.rs) | <n/a> | suggestion | code-quality | All domain binaries use port 8081 — should use configurable ports | Use `env::var` with a default of 8081 for each domain's port | deferred: Minor — configurable ports are a Phase 1+ concern |
| coverage | Phase 4 (docker-compose) | <n/a> | suggestion | verification-coverage | Frontend Dockerfiles not scaffolded — `docker compose up` will fail for frontend services | Document that frontend images are out of scope; `docker compose up gateway` works independently | applied: Verification steps focus on gateway only; frontend services are explicitly out of scope |

## References

- Design: `.rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md`
- Solutions: `.rpiv/artifacts/solutions/rust-microservices-architecture.md`
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- axum Router::nest(): https://docs.rs/axum/latest/axum/struct.Router.html#method.nest
- thiserror: https://docs.rs/thiserror/
- SQLite WAL mode: https://www.sqlite.org/wal.html
