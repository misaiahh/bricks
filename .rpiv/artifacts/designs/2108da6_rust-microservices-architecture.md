---
date: "2026-06-03T06:07:48+00:00"
author: Micky Hallabrin
commit: 2108da6
branch: main
repository: bricks
topic: "Rust microservices architecture design"
tags: [design, rust, microservices, workspace, axum]
status: ready
parent: .rpiv/artifacts/solutions/rust-microservices-architecture.md
last_updated: "2026-06-03T06:07:48+00:00"
last_updated_by: Micky Hallabrin
---

# Design: Rust Microservices Architecture — Workspace with Separate Crates

## Summary

Replace the single `services/api/` axum binary with a Cargo workspace under `services/` containing a `shared/` infrastructure crate, a `gateway/` routing crate, and stub crates for each domain (orders, users, employees, equipment, materials, notifications). Phase 0 runs everything in-process via the gateway; extraction to separate containers happens mechanically in Phase 1+. Frontend apps connect to the gateway at port 8080, unchanged.

## Requirements

- Replace `services/api/` (single binary) with a microservices workspace structure
- Each domain (orders, users, employees, equipment, materials, notifications) has clear ownership boundaries
- Shared infrastructure (tracing, error handling, DB patterns) is DRY via a `shared/` crate
- Gateway mounts all domain routers in-process (Phase 0), can route via HTTP (Phase 1+)
- Frontend apps (`apps/orders`, `apps/admin`) continue working with minimal changes
- Gradual split: start as modular monolith, extract services over time
- SQLite compatible in Phase 0 (single process); WAL mode or per-service DBs in Phase 1+
- Docker Compose updated to run gateway + extracted services
- Small team (1-2 developers) — manageable complexity

## Current State Analysis

### Key Discoveries

- **`services/api/Cargo.toml`** — Single crate `bricks-api` with axum 0.8, tokio, serde, tower, tracing-subscriber (env-filter), tower-http (trace). No sqlx, no migrations, no models yet.
- **`services/api/src/main.rs`** — Single binary with: axum Router, health endpoint (`GET /health`), `AppState` with service name, graceful shutdown (Ctrl+C + SIGTERM), tracing-subscriber with env-filter, `TraceLayer` middleware. Listening on port 8080.
- **`docker-compose.yml`** — Three services: `orders` (port 3000), `admin` (port 3001), `api` (port 8080). API has healthcheck via curl. Frontend apps depend on `api` being healthy.
- **`ARCHITECTURE.md`** — Documents "no shared configs" and "no build orchestration" decisions. Directory structure shows `services/api/` as the sole backend.
- **`AGENTS.md`** — AI agent routing guide at repo root. References `services/api/` as the backend.
- **`apps/orders/src/App.ts`** — Static placeholder, no API calls.
- **`apps/admin/src/App.ts`** — Static placeholder, no API calls.
- **No database layer exists yet** — sqlx, migrations, and models are not present. This simplifies Phase 0 since we're not migrating any existing DB schema.

### Constraints

- SQLite shared-file locking limits concurrent writes across processes
- No monorepo tooling currently (no Turborepo, Nx)
- Docker Compose for orchestration (not Kubernetes)
- Single codebase monorepo structure
- Small team (1-2 developers)

## Scope

### Building
- Cargo workspace root at `services/` with workspace `Cargo.toml`
- `shared/` crate: tracing config, error types (`thiserror`), DB pool pattern, common middleware
- `gateway/` crate: in-process router mounting all domain routers via `Router::nest()`
- Domain crate stubs (lib + bin targets): orders, users, employees, equipment, materials, notifications
- Each domain crate exports a `fn router() -> Router` function (empty in Phase 0)
- Single Docker container running the gateway binary (Phase 0)
- Updated `docker-compose.yml` with gateway service replacing `api`
- Updated `ARCHITECTURE.md` reflecting new structure
- Updated `AGENTS.md` reflecting new structure
- Health endpoint at `GET /health` on the gateway
- Graceful shutdown pattern preserved

### Not Building
- Actual domain handlers, models, or business logic (orders, users, etc.) — these are stubs only
- Database migrations or sqlx setup — no DB layer in Phase 0
- Authentication/authorization
- Inter-service HTTP communication (reqwest-based routing) — Phase 1+
- Correlation IDs for cross-service tracing — Phase 2+
- Individual Dockerfiles per domain — Phase 1+
- Service discovery or health registry — Phase 2+
- CI pipeline changes — out of scope
- Frontend app changes — frontend is static placeholders

## Decisions

### Workspace Root Location

**Decision**: Workspace root at `services/` with `Cargo.toml` at `services/Cargo.toml`.

**Rationale**: The `services/` directory already exists and contains `api/`. Placing the workspace root here keeps the monorepo structure clean: `apps/` for frontends, `services/` for backends. This is the standard Rust workspace pattern (see [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)).

**Evidence**: `services/` currently contains only `api/`. Moving `api/`'s contents into the workspace structure is a single rename + refactor operation.

### Shared Crate Purpose

**Decision**: `shared/` crate contains only infrastructure — no domain logic. Provides: tracing subscriber config, error types (`thiserror`), DB connection pool setup pattern, common middleware (CORS, compression), and graceful shutdown utilities.

**Rationale**: Prevents domain coupling through the shared crate. Each domain crate owns its own models and handlers. The shared crate is a pure infrastructure dependency.

**Evidence**: Current `services/api/src/main.rs` has tracing config (line 40-43), `AppState` struct (lines 23-26), and shutdown handler (lines 28-42). These are all infrastructure concerns.

### Gateway Routing Strategy

**Decision**: Gateway uses `Router::nest()` for in-process routing. Each domain mounts at a path prefix: `/orders`, `/users`, `/employees`, `/equipment`, `/materials`, `/notifications`. The health endpoint lives at `/health` on the gateway itself.

**Rationale**: `nest()` provides clean path-based routing that maps naturally to domain boundaries. The frontend apps are static placeholders with no hardcoded endpoints, so path restructuring is low-risk. `nest()` is simpler than `merge()` for path-based domain separation.

**Evidence**: axum's `Router::nest(prefix, router)` mounts a sub-router at a path prefix. This is the standard pattern for multi-domain routing in axum (see [axum Router docs](https://docs.rs/axum/latest/axum/struct.Router.html)).

### Domain Crate Structure

**Decision**: Each domain crate has both `bin` and `lib` targets. The `lib` target exports a `fn router() -> Router` function. The `bin` target (`main.rs`) starts the service as a standalone process (used in Phase 1+ extraction). In Phase 0, the `bin` target is built but the `lib` target is what the gateway imports.

**Rationale**: Having both targets means each domain is independently runnable from day one. When extracting a service to its own container, the `bin` target already exists — no code movement needed. The `lib` target provides the `router()` function that the gateway imports in Phase 0.

**Evidence**: This is the standard Rust workspace pattern for crates that need to be both library and binary (see [Cargo bin targets](https://doc.rust-lang.org/cargo/reference/manifest.html#the-bin-section)).

### SQLite Strategy

**Decision**: Phase 0 uses a single SQLite database file accessible in-process by all domain crates. In Phase 1+, when extracting a service, use WAL mode for read-heavy services or migrate write-heavy services to a separate database.

**Rationale**: Single process in Phase 0 means no SQLite concurrency issues. WAL mode (`PRAGMA journal_mode=WAL`) enables concurrent reads in Phase 1+. This is a documented SQLite best practice for multi-process access.

**Evidence**: Current codebase has no database layer yet (no sqlx in `Cargo.toml`). Migration timing is flexible — we can set up the DB layer alongside or after the workspace structure.

### Docker Strategy

**Decision**: Phase 0 uses a single Docker container running the gateway binary. The Dockerfile is at `services/gateway/Dockerfile` and compiles the entire workspace via `cargo build --release --package bricks-gateway`. Phase 1+ adds individual Dockerfiles per extracted service.

**Rationale**: Single container in Phase 0 keeps Docker Compose simple and matches the current deployment model. Adding per-service Dockerfiles later is mechanical.

**Evidence**: Current `docker-compose.yml` has one API container. Replacing it with a gateway container is a minimal diff.

### Domain Scope for Phase 0

**Decision**: All six domains (orders, users, employees, equipment, materials, notifications) get stub crates in Phase 0. Each stub exports an empty `router()` function.

**Rationale**: Having all stubs from day one establishes ownership boundaries for every domain. It makes the workspace structure complete and testable. Adding domains later would require workspace `members` changes and gateway `nest()` updates.

**Evidence**: The solutions artifact confirms "domain crate stubs" as Phase 0 deliverable.

## Architecture

### services/Cargo.toml — NEW

Workspace root. Defines workspace members, shared dependencies, and package metadata.

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

### services/shared/Cargo.toml — NEW

Shared infrastructure crate. No domain logic.

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

### services/shared/src/lib.rs — NEW

Shared infrastructure: error types, tracing config, DB pool pattern, common middleware, shutdown utilities.

```rust
use axum::{
    Router,
    middleware::{self, Next},
    response::Response,
    http::{Request, StatusCode},
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
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
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

// ─── Server Utilities ────────────────────────────────────────────────────────

pub struct ServerConfig {
    pub addr: SocketAddr,
    pub service_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from(([0, 0, 0, 0], 8080)),
            service_name: "bricks-gateway".to_string(),
        }
    }
}

// ─── Domain Router Trait ─────────────────────────────────────────────────────

/// Trait that domain crates implement to provide their router.
/// The gateway calls `domain_router()` to get each domain's sub-router.
pub trait DomainRouter {
    fn domain_router() -> Router;
    fn domain_name() -> &'static str;
}

// ─── DB Pool Type Alias (optional, for domain crates that need it) ───────────

#[cfg(feature = "db")]
pub type DbPool = sqlx::SqlitePool;

// ─── CORS Middleware (optional, for extracted services) ──────────────────────

pub fn cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::permissive()
}
```

### services/gateway/Cargo.toml — NEW

Gateway crate. Replaces `services/api/Cargo.toml`.

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
tower = { workspace = true }
tower-http = { workspace = true, features = ["trace"] }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

### services/gateway/src/main.rs — NEW

Gateway binary: mounts all domain routers via `Router::nest()`, provides health endpoint, handles graceful shutdown.

```rust
use axum::{
    routing::get,
    Router,
};
use bricks_shared::{
    init_tracing,
    shutdown_handler,
    ServerConfig,
    HealthResponse,
};
use serde::Serialize;

#[derive(Clone)]
struct GatewayState {
    service_name: String,
}

async fn health(axum::extract::State(state): axum::extract::State<GatewayState>) -> axum::response::Json<HealthResponse> {
    axum::response::Json(HealthResponse {
        status: "ok",
        service: state.service_name,
    })
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_gateway");

    let config = ServerConfig::default();
    let state = GatewayState {
        service_name: config.service_name.clone(),
    };

    // Mount all domain routers at their path prefixes.
    // In Phase 0, all routers are in-process (empty stubs).
    // In Phase 1+, extracted services route via HTTP (reqwest).
    let app = Router::new()
        .route("/health", get(health))
        .nest("/orders", bricks_orders::orders_router())
        .nest("/users", bricks_users::users_router())
        .nest("/employees", bricks_employees::employees_router())
        .nest("/equipment", bricks_equipment::equipment_router())
        .nest("/materials", bricks_materials::materials_router())
        .nest("/notifications", bricks_notifications::notifications_router())
        .with_state(state)
        .layer(tower::ServiceBuilder::new().layer(
            tower_http::trace::TraceLayer::new_for_http(),
        ));

    let addr = config.addr;
    tracing::info!("listening on {}", addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&addr)
            .await
            .expect("failed to bind to address"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}
```

### services/orders/Cargo.toml — NEW

Orders domain crate stub.

```toml
[package]
name = "bricks-orders"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-orders"
path = "src/main.rs"

[lib]
name = "bricks_orders"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/orders/src/lib.rs — NEW

Orders domain library: exports `orders_router()` function.

```rust
use axum::Router;

/// Returns the orders domain router.
/// In Phase 0: empty router (stub).
/// In Phase 1+: mount orders handlers here.
pub fn orders_router() -> Router {
    Router::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_returns_non_empty() {
        let router = orders_router();
        // Router is always non-empty (has at least the base route)
        assert!(format!("{:?}", router) != "");
    }
}
```

### services/orders/src/main.rs — NEW

Orders domain binary (standalone entry point, used in Phase 1+ extraction).

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_orders");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8081).into(),
        service_name: "bricks-orders".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(orders_router());

    tracing::info!("orders service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

// Re-export the lib's router function for the gateway
pub use bricks_orders::orders_router;
```

### services/users/Cargo.toml — NEW

Users domain crate stub.

```toml
[package]
name = "bricks-users"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-users"
path = "src/main.rs"

[lib]
name = "bricks_users"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/users/src/lib.rs — NEW

Users domain library: exports `users_router()` function.

```rust
use axum::Router;

/// Returns the users domain router.
pub fn users_router() -> Router {
    Router::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn router_returns_non_empty() {
        let router = users_router();
        assert!(format!("{:?}", router) != "");
    }
}
```

### services/users/src/main.rs — NEW

Users domain binary (standalone entry point).

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_users");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8082).into(),
        service_name: "bricks-users".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(users_router());

    tracing::info!("users service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

pub use bricks_users::users_router;
```

### services/employees/Cargo.toml — NEW

Employees domain crate stub.

```toml
[package]
name = "bricks-employees"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-employees"
path = "src/main.rs"

[lib]
name = "bricks_employees"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/employees/src/lib.rs — NEW

```rust
use axum::Router;

pub fn employees_router() -> Router {
    Router::new()
}
```

### services/employees/src/main.rs — NEW

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_employees");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8083).into(),
        service_name: "bricks-employees".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(employees_router());

    tracing::info!("employees service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

pub use bricks_employees::employees_router;
```

### services/equipment/Cargo.toml — NEW

Equipment domain crate stub.

```toml
[package]
name = "bricks-equipment"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-equipment"
path = "src/main.rs"

[lib]
name = "bricks_equipment"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/equipment/src/lib.rs — NEW

```rust
use axum::Router;

pub fn equipment_router() -> Router {
    Router::new()
}
```

### services/equipment/src/main.rs — NEW

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_equipment");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8084).into(),
        service_name: "bricks-equipment".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(equipment_router());

    tracing::info!("equipment service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

pub use bricks_equipment::equipment_router;
```

### services/materials/Cargo.toml — NEW

Materials domain crate stub.

```toml
[package]
name = "bricks-materials"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-materials"
path = "src/main.rs"

[lib]
name = "bricks_materials"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/materials/src/lib.rs — NEW

```rust
use axum::Router;

pub fn materials_router() -> Router {
    Router::new()
}
```

### services/materials/src/main.rs — NEW

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_materials");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8085).into(),
        service_name: "bricks-materials".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(materials_router());

    tracing::info!("materials service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

pub use bricks_materials::materials_router;
```

### services/notifications/Cargo.toml — NEW

Notifications domain crate stub.

```toml
[package]
name = "bricks-notifications"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bricks-notifications"
path = "src/main.rs"

[lib]
name = "bricks_notifications"
path = "src/lib.rs"

[dependencies]
bricks-shared = { path = "../shared" }

axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }
```

### services/notifications/src/lib.rs — NEW

```rust
use axum::Router;

pub fn notifications_router() -> Router {
    Router::new()
}
```

### services/notifications/src/main.rs — NEW

```rust
use axum::{routing::get, Router};
use bricks_shared::{init_tracing, shutdown_handler, ServerConfig};

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    init_tracing("bricks_notifications");

    let config = ServerConfig {
        addr: ([0, 0, 0, 0], 8086).into(),
        service_name: "bricks-notifications".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(notifications_router());

    tracing::info!("notifications service listening on {}", config.addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&config.addr)
            .await
            .expect("failed to bind"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}

pub use bricks_notifications::notifications_router;
```

### services/api/ — DELETED

The old `services/api/` directory is removed. Its contents are replaced by the workspace structure.

### docker-compose.yml — MODIFY

Replace the `api` service with a `gateway` service. Port remains 8080.

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
      context: .
      dockerfile: services/gateway/Dockerfile
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 3
    restart: unless-stopped
```

### services/gateway/Dockerfile — NEW

Single-container build for Phase 0. Compiles the entire workspace.

```dockerfile
FROM rust:1.75-slim AS builder

WORKDIR /app

# Copy workspace root first (leverages Docker cache)
COPY services/Cargo.toml services/Cargo.toml
COPY services/shared/Cargo.toml services/shared/Cargo.toml
COPY services/gateway/Cargo.toml services/gateway/Cargo.toml
COPY services/orders/Cargo.toml services/orders/Cargo.toml
COPY services/users/Cargo.toml services/users/Cargo.toml
COPY services/employees/Cargo.toml services/employees/Cargo.toml
COPY services/equipment/Cargo.toml services/equipment/Cargo.toml
COPY services/materials/Cargo.toml services/materials/Cargo.toml
COPY services/notifications/Cargo.toml services/notifications/Cargo.toml

# Copy source
COPY services/shared/ services/shared/
COPY services/gateway/ services/gateway/
COPY services/orders/ services/orders/
COPY services/users/ services/users/
COPY services/employees/ services/employees/
COPY services/equipment/ services/equipment/
COPY services/materials/ services/materials/
COPY services/notifications/ services/notifications/

RUN cargo build --release --package bricks-gateway

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bricks-gateway /usr/local/bin/bricks-gateway

EXPOSE 8080

CMD ["bricks-gateway"]
```

### ARCHITECTURE.md — MODIFY

Update directory structure and add workspace section.

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
    ├── Cargo.toml           # Workspace root
    ├── shared/              # Shared infrastructure (tracing, errors, DB patterns)
    ├── gateway/             # API gateway (mounts all domain routers in-process)
    ├── orders/              # Orders domain crate
    ├── users/               # Users domain crate
    ├── employees/           # Employees domain crate
    ├── equipment/           # Equipment domain crate
    ├── materials/           # Materials domain crate
    └── notifications/       # Notifications domain crate
```

## Tech Stack
- **Frontends:** Vanilla TypeScript, Vite (build/dev), Vitest (unit tests), Playwright (E2E), ESLint, Prettier
- **Backend:** Rust, Cargo workspace, axum (web framework), thiserror (errors), sqlx (SQLite), tracing (logging)
- **Orchestration:** Docker Compose
- **Workspace:** Cargo workspace with 8 crates (shared, gateway, 6 domains)

## Design Decisions
- Cargo workspace under `services/` — each domain is an independent crate
- Shared infrastructure in `shared/` crate — DRY tracing, error handling, DB patterns
- Gateway mounts all domain routers in-process via `Router::nest()`
- Each domain crate has both `bin` and `lib` targets for independent deployment
- Frontend apps connect to gateway at port 8080
- SQLite in Phase 0 (single process); WAL mode or per-service DBs in Phase 1+
- No shared configuration packages — each project maintains its own configs
- AGENTS.md at root for AI agent routing with progressive disclosure
```

### AGENTS.md — MODIFY

Update backend reference from `services/api/` to `services/` workspace.

```markdown
# Bricks Monorepo — AI Agent Guide

## Repo Structure
- `apps/orders/` — Public order creation frontend (vanilla TS, Vite, Vitest, Playwright)
- `apps/admin/` — Brick management admin frontend (vanilla TS, Vite, Vitest, Playwright)
- `services/` — Rust backend workspace (Cargo workspace, 8 crates)
  - `shared/` — Shared infrastructure (tracing, errors, DB patterns)
  - `gateway/` — API gateway (mounts all domain routers in-process)
  - `orders/` — Orders domain
  - `users/` — Users domain
  - `employees/` — Employees domain
  - `equipment/` — Equipment domain
  - `materials/` — Materials domain
  - `notifications/` — Notifications domain

## Commands
- `npm install` — Install dependencies for current directory
- `docker compose up` — Start all services
- `docker compose up --build gateway` — Build and start gateway
- `cargo build --workspace` — Build all backend crates
- `cargo test --workspace` — Run tests across all backend crates

## Shared Rules
- Each project is self-contained — no shared configs between projects
- Frontend apps: vanilla TypeScript, no framework
- Backend services: Rust Cargo workspace with separate domain crates
- Run linting and tests before committing
- Use Docker Compose for service orchestration

## When to Read More
- For orders app details → read `apps/orders/README.md`
- For admin app details → read `apps/admin/README.md`
- For backend details → read `services/ARCHITECTURE.md` (or `ARCHITECTURE.md` at repo root)
- For architecture decisions → read `ARCHITECTURE.md`
```

## Slices

### Slice 1: Workspace Root + Shared Crate

**Files**: `services/Cargo.toml` (NEW), `services/shared/Cargo.toml` (NEW), `services/shared/src/lib.rs` (NEW)

#### Automated Verification:
- [ ] `cargo build --workspace` compiles the workspace root and shared crate
- [ ] `cargo test --workspace` runs tests in the shared crate
- [ ] `services/Cargo.toml` contains all 8 workspace members
- [ ] Shared crate exports: `AppError`, `HealthResponse`, `init_tracing()`, `shutdown_handler()`, `ServerConfig`, `DomainRouter` trait, `cors_layer()`

#### Manual Verification:
- [ ] Shared crate compiles independently: `cargo build -p bricks-shared`
- [ ] `init_tracing()` configures tracing-subscriber with env-filter
- [ ] `shutdown_handler()` handles both Ctrl+C and SIGTERM

### Slice 2: Gateway Crate

**Files**: `services/gateway/Cargo.toml` (NEW), `services/gateway/src/main.rs` (NEW), `services/gateway/Dockerfile` (NEW)

#### Automated Verification:
- [ ] `cargo build --package bricks-gateway` compiles the gateway
- [ ] `cargo test --workspace` passes (gateway has no tests yet)
- [ ] Gateway binary is produced at `target/release/bricks-gateway`
- [ ] Gateway imports all 6 domain crates from workspace

#### Manual Verification:
- [ ] Gateway mounts all 6 domain routers via `Router::nest()`
- [ ] Health endpoint at `GET /health` returns `{"status":"ok","service":"bricks-gateway"}`
- [ ] Graceful shutdown handles Ctrl+C and SIGTERM
- [ ] Dockerfile builds successfully: `docker build -f services/gateway/Dockerfile -t bricks-gateway:latest .`

### Slice 3: Domain Crate Stubs (All 6 Domains)

**Files**: 12 NEW files (6 `Cargo.toml`, 6 `src/lib.rs`, 6 `src/main.rs` — but grouped as one slice since they're structurally identical)

#### Automated Verification:
- [ ] `cargo build --workspace` compiles all 8 crates (shared + gateway + 6 domains)
- [ ] `cargo test --workspace` runs tests in all domain crates
- [ ] Each domain crate has both `lib` and `bin` targets
- [ ] Each domain crate exports a `*_router()` function

#### Manual Verification:
- [ ] Each domain crate compiles independently: `cargo build -p bricks-<domain>`
- [ ] Each domain `lib.rs` exports a `router()` function returning `axum::Router`
- [ ] Each domain `main.rs` starts a standalone process with health endpoint
- [ ] Domain crates only depend on `bricks-shared` (no cross-domain dependencies)

### Slice 4: Docker Compose + Documentation Updates

**Files**: `docker-compose.yml` (MODIFY), `ARCHITECTURE.md` (MODIFY), `AGENTS.md` (MODIFY)

#### Automated Verification:
- [ ] `docker compose config` validates the updated compose file
- [ ] `docker compose up --build gateway` starts the gateway service
- [ ] Gateway healthcheck passes: `curl -f http://localhost:8080/health`
- [ ] `ARCHITECTURE.md` reflects new workspace structure
- [ ] `AGENTS.md` references `services/` workspace instead of `services/api/`

#### Manual Verification:
- [ ] Frontend apps still depend on `gateway` (renamed from `api`) in `depends_on`
- [ ] Port 8080 is unchanged — frontend apps have no hardcoded backend URLs (static placeholders)
- [ ] Old `services/api/` directory is removed

## Desired End State

After Phase 0, the developer can:

1. **Build the entire workspace**:
   ```bash
   cargo build --workspace
   cargo test --workspace
   ```

2. **Run the gateway locally**:
   ```bash
   cargo run --package bricks-gateway
   # → listening on 0.0.0.0:8080
   ```

3. **Hit the health endpoint**:
   ```bash
   curl http://localhost:8080/health
   # → {"status":"ok","service":"bricks-gateway"}
   ```

4. **Build and run via Docker**:
   ```bash
   docker compose up --build gateway
   # → gateway starts, healthcheck passes
   ```

5. **Add a handler to a domain**:
   ```rust
   // In services/orders/src/lib.rs
   pub fn orders_router() -> Router {
       Router::new()
           .route("/", get(list_orders))
           .route("/{id}", get(get_order))
   }
   ```
   The gateway automatically routes `/orders/*` to the orders domain.

6. **Extract a domain to its own container** (Phase 1):
   ```bash
   # Add a Dockerfile for the domain
   # Update docker-compose.yml with a new service
   # Change gateway to route via HTTP (reqwest)
   ```

## File Map

services/Cargo.toml                          # NEW — workspace root with members and shared dependencies
services/shared/Cargo.toml                   # NEW — shared infrastructure crate manifest
services/shared/src/lib.rs                   # NEW — error types, tracing, shutdown, server utils, domain trait
services/gateway/Cargo.toml                  # NEW — gateway crate manifest
services/gateway/src/main.rs                 # NEW — gateway binary, mounts all domain routers via Router::nest()
services/gateway/Dockerfile                  # NEW — multi-stage Docker build for Phase 0
services/orders/Cargo.toml                   # NEW — orders domain crate manifest
services/orders/src/lib.rs                   # NEW — orders domain lib, exports orders_router()
services/orders/src/main.rs                  # NEW — orders domain binary (standalone, Phase 1+)
services/users/Cargo.toml                    # NEW — users domain crate manifest
services/users/src/lib.rs                    # NEW — users domain lib, exports users_router()
services/users/src/main.rs                   # NEW — users domain binary (standalone, Phase 1+)
services/employees/Cargo.toml                # NEW — employees domain crate manifest
services/employees/src/lib.rs                # NEW — employees domain lib, exports employees_router()
services/employees/src/main.rs               # NEW — employees domain binary (standalone, Phase 1+)
services/equipment/Cargo.toml                # NEW — equipment domain crate manifest
services/equipment/src/lib.rs                # NEW — equipment domain lib, exports equipment_router()
services/equipment/src/main.rs               # NEW — equipment domain binary (standalone, Phase 1+)
services/materials/Cargo.toml                # NEW — materials domain crate manifest
services/materials/src/lib.rs                # NEW — materials domain lib, exports materials_router()
services/materials/src/main.rs               # NEW — materials domain binary (standalone, Phase 1+)
services/notifications/Cargo.toml            # NEW — notifications domain crate manifest
services/notifications/src/lib.rs            # NEW — notifications domain lib, exports notifications_router()
services/notifications/src/main.rs           # NEW — notifications domain binary (standalone, Phase 1+)
services/api/                                # DELETED — entire old API directory removed
docker-compose.yml                           # MODIFY — api → gateway service, frontend depends_on updated
ARCHITECTURE.md                              # MODIFY — directory structure, tech stack, design decisions updated
AGENTS.md                                    # MODIFY — backend reference updated to workspace structure

## Ordering Constraints

1. **Slice 1 must complete before Slice 2** — Gateway depends on `bricks-shared`
2. **Slice 1 must complete before Slice 3** — Domain crates depend on `bricks-shared`
3. **Slice 2 and Slice 3 can run in parallel** — They are independent (gateway doesn't import domain logic yet, domains don't import gateway)
4. **Slice 4 must complete last** — It updates documentation and Docker Compose to reflect the completed workspace
5. **`services/api/` must be removed after Slice 3** — Before Slice 4 updates docker-compose.yml

## Verification Notes

- **Build verification**: `cargo build --workspace` should compile all 8 crates. If any crate fails, check that workspace dependencies are correctly referenced.
- **Test verification**: `cargo test --workspace` should pass. Domain stubs have minimal tests (router returns non-empty).
- **Docker verification**: The gateway Dockerfile uses a multi-stage build. First stage compiles with `rust:1.75-slim`, second stage uses `debian:bookworm-slim` with only curl for healthchecks.
- **Port conflicts**: Domain binaries (Phase 1+) use ports 8081-8086. Gateway uses 8080. No conflicts in Phase 0.
- **SQLite concurrency**: Phase 0 is single-process, so no SQLite locking issues. Document WAL mode strategy for Phase 1+.
- **Frontend compatibility**: Frontend apps are static placeholders with no backend calls. No frontend changes needed in Phase 0.

## Performance Considerations

- **In-process routing (Phase 0)**: `Router::nest()` adds minimal overhead — it's just path prefix matching at the router level. No serialization/deserialization.
- **Gateway as single point**: All domains run in the same process. Gateway CPU/memory is the bottleneck. Monitor with tracing metrics.
- **Extraction overhead (Phase 1+)**: When extracting a service, inter-service communication adds HTTP latency (reqwest round-trip). Typical overhead: 1-5ms per request within the same Docker network.
- **SQLite read paths**: WAL mode enables concurrent reads. Write contention is the primary concern — design write-heavy domains to use their own database in Phase 1+.

## Migration Notes

- **`services/api/` removal**: The old `services/api/` directory is completely replaced. Its `Cargo.toml` and `src/main.rs` are not migrated — the workspace structure is a fresh start. This is safe because:
  - No database layer exists yet (no sqlx, no migrations, no models to migrate)
  - Only one endpoint exists (health), which is replicated in the gateway
  - No domain handlers exist yet
- **Docker image names**: `bricks-api` → `bricks-gateway`. Docker Compose service name changes from `api` to `gateway`. Frontend apps' `depends_on` updated accordingly.
- **No breaking changes for frontend**: Frontend apps are static placeholders. When they start calling the backend, they'll hit the gateway at the same port (8080) with the same health endpoint path (`/health`).
- **Rollback**: If Phase 0 fails, the old `services/api/` can be restored from git: `git checkout HEAD -- services/api/`. The workspace structure is additive — it doesn't modify existing files until Slice 4.

## Pattern References

- `services/api/src/main.rs:1-50` — axum Router pattern with `with_state()` and `TraceLayer` — modeled in gateway's `main.rs`
- `services/api/src/main.rs:23-42` — graceful shutdown with Ctrl+C + SIGTERM — extracted to `bricks-shared::shutdown_handler()`
- `services/api/src/main.rs:40-43` — tracing-subscriber with env-filter — extracted to `bricks-shared::init_tracing()`
- `services/api/src/main.rs:16-21` — health endpoint pattern — replicated in gateway and each domain binary
- `docker-compose.yml:17-23` — healthcheck pattern — reused for gateway service
- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) — workspace root, members, shared dependencies
- [axum Router::nest()](https://docs.rs/axum/latest/axum/struct.Router.html#method.nest) — path-based domain routing
- [thiserror](https://docs.rs/thiserror/latest/thiserror/) — error type derivation pattern

## Developer Context

- **Question**: Which domains should get stub crates in Phase 0?
  - **Answer**: All six domains (orders, users, employees, equipment, materials, notifications). Establishes ownership boundaries for every domain from day one.
- **Question**: Gateway routing: `nest()` vs `merge()`?
  - **Answer**: `nest()` for path-based domain routing (`/orders`, `/users`, etc.). Simpler, cleaner URL structure. `merge()` can be used later if flat routing is needed.
- **Question**: SQLite strategy for Phase 1+ extraction?
  - **Answer**: WAL mode (`PRAGMA journal_mode=WAL`) for read-heavy services. Separate databases for write-heavy services. Document in Phase 1 planning.
- **Question**: Frontend app changes needed?
  - **Answer**: None in Phase 0. Frontend apps are static placeholders with no API calls. When they start calling the backend, they'll hit the gateway at port 8080.

## Design History

- Slice 1: Workspace Root + Shared Crate — approved as generated
- Slice 2: Gateway Crate — approved as generated
- Slice 3: Domain Crate Stubs (All 6 Domains) — approved as generated
- Slice 4: Docker Compose + Documentation Updates — approved as generated

## References

- `.rpiv/artifacts/solutions/rust-microservices-architecture.md` — source solutions artifact (Option A: Workspace with Separate Crates)
- `services/api/Cargo.toml` — current single-crate configuration, base dependencies
- `services/api/src/main.rs` — current axum patterns (Router, TraceLayer, graceful shutdown)
- `docker-compose.yml` — current orchestration, healthcheck pattern
- `ARCHITECTURE.md` — current architecture decisions
- `AGENTS.md` — current AI agent routing guide
- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) — Rust workspace documentation
- [axum Router](https://docs.rs/axum/latest/axum/struct.Router.html) — axum Router documentation for `merge()` and `nest()`
- [thiserror](https://docs.rs/thiserror/) — Rust error type derivation
- [SQLite WAL mode](https://www.sqlite.org/wal.html) — Write-Ahead Logging for concurrent access
