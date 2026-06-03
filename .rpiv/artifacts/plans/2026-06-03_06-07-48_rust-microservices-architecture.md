---
date: "2026-06-03T06:07:48+00:00"
author: Micky Hallabrin
commit: 2108da6
branch: main
repository: bricks
topic: "Rust microservices architecture implementation"
tags: [plan, rust, microservices, workspace, axum]
status: in-review
parent: ".rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md"
last_updated: "2026-06-03T06:07:48+00:00"
last_updated_by: Micky Hallabrin
---

# Rust Microservices Architecture Implementation Plan

## Overview

Replace the single `services/api/` axum binary with a Cargo workspace under `services/` containing a `shared/` infrastructure crate, a `gateway/` routing crate, and stub crates for each domain (orders, users, employees, equipment, materials, notifications). Phase 0 runs everything in-process via the gateway; extraction to separate containers happens mechanically in Phase 1+. Frontend apps connect to the gateway at port 8080, unchanged.

Derived from design artifact: `.rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md`.

## Desired End State

After Phase 0, the developer can:

1. **Build the entire workspace**: `cargo build --workspace` and `cargo test --workspace`
2. **Run the gateway locally**: `cargo run --package bricks-gateway` → listening on 0.0.0.0:8080
3. **Hit the health endpoint**: `curl http://localhost:8080/health` → `{"status":"ok","service":"bricks-gateway"}`
4. **Build and run via Docker**: `docker compose up --build gateway` → gateway starts, healthcheck passes
5. **Add a handler to a domain**: Write a handler in a domain's `lib.rs`, the gateway auto-routes it
6. **Extract a domain to its own container** (Phase 1): Add a Dockerfile, update docker-compose.yml, change gateway to route via HTTP

## What We're NOT Doing

- Actual domain handlers, models, or business logic (orders, users, etc.) — these are stubs only
- Database migrations or sqlx setup — no DB layer in Phase 0
- Authentication/authorization
- Inter-service HTTP communication (reqwest-based routing) — Phase 1+
- Correlation IDs for cross-service tracing — Phase 2+
- Individual Dockerfiles per domain — Phase 1+
- Service discovery or health registry — Phase 2+
- CI pipeline changes — out of scope
- Frontend app changes — frontend is static placeholders

## Phase 1: Workspace Root + Shared Crate

### Overview

Create the Cargo workspace root at `services/Cargo.toml` and the `shared/` infrastructure crate. This establishes the foundation that all other crates depend on. The shared crate provides error types (`thiserror`), tracing configuration, graceful shutdown, `ServerConfig`, the `DomainRouter` trait, and CORS middleware.

### Changes Required:

#### 1. Workspace Root Cargo.toml
**File**: `services/Cargo.toml` (NEW)
**Changes**: Create workspace root defining 8 members (shared, gateway, 6 domains), shared dependencies, workspace package metadata, and clippy lints.

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

#### 2. Shared Crate Manifest
**File**: `services/shared/Cargo.toml` (NEW)
**Changes**: Create shared infrastructure crate manifest referencing workspace dependencies. Mark `sqlx`, `reqwest`, and `uuid` as optional.

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

#### 3. Shared Crate Library
**File**: `services/shared/src/lib.rs` (NEW)
**Changes**: Implement shared infrastructure: `AppError` types with `IntoResponse`, `HealthResponse` struct, `init_tracing()` function, `shutdown_handler()` async function, `ServerConfig` struct with defaults, `DomainRouter` trait, `DbPool` type alias (cfg-gated), and `cors_layer()` function.

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

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --workspace` compiles the workspace root and shared crate
- [ ] `cargo test --workspace` runs tests in the shared crate
- [ ] `services/Cargo.toml` contains all 8 workspace members
- [ ] Shared crate exports: `AppError`, `HealthResponse`, `init_tracing()`, `shutdown_handler()`, `ServerConfig`, `DomainRouter` trait, `cors_layer()`

#### Manual Verification:
- [ ] Shared crate compiles independently: `cargo build -p bricks-shared`
- [ ] `init_tracing()` configures tracing-subscriber with env-filter
- [ ] `shutdown_handler()` handles both Ctrl+C and SIGTERM

---

## Phase 2: Gateway Crate

### Overview

Create the `gateway/` crate that replaces `services/api/`. The gateway binary mounts all 6 domain routers via `Router::nest()`, provides the `/health` endpoint, and handles graceful shutdown. Also create the Dockerfile for the single-container Phase 0 build.

### Changes Required:

#### 1. Gateway Crate Manifest
**File**: `services/gateway/Cargo.toml` (NEW)
**Changes**: Create gateway crate manifest with `bin` target `bricks-gateway`. Depends on `bricks-shared` and all 6 domain crates.

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

#### 2. Gateway Binary
**File**: `services/gateway/src/main.rs` (NEW)
**Changes**: Gateway binary that mounts all domain routers via `Router::nest()`, provides `/health` endpoint, handles graceful shutdown. Uses `ServerConfig` from shared crate.

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

#### 3. Gateway Dockerfile
**File**: `services/gateway/Dockerfile` (NEW)
**Changes**: Multi-stage Docker build. Stage 1 (`rust:1.75-slim`) compiles the workspace. Stage 2 (`debian:bookworm-slim`) contains only the binary and curl for healthchecks.

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

### Success Criteria:

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

---

## Phase 3: Domain Crate Stubs (All 6 Domains)

### Overview

Create stub crates for all 6 domains: orders, users, employees, equipment, materials, notifications. Each domain crate has both `lib` and `bin` targets. The `lib` target exports a `*_router()` function returning an empty `axum::Router`. The `bin` target starts the service as a standalone process (used in Phase 1+ extraction). All domain crates only depend on `bricks-shared` — no cross-domain dependencies.

### Changes Required:

#### 1. Orders Domain Crate
**File**: `services/orders/Cargo.toml` (NEW)
**Changes**: Orders domain crate with `lib` and `bin` targets. Depends on `bricks-shared`, `axum`, `tokio`, `serde`, `tracing`.

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

**File**: `services/orders/src/lib.rs` (NEW)
**Changes**: Orders domain library exporting `orders_router()` function with a test.

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

**File**: `services/orders/src/main.rs` (NEW)
**Changes**: Orders domain binary — standalone entry point with health endpoint and graceful shutdown.

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

pub use bricks_orders::orders_router;
```

#### 2. Users Domain Crate
**File**: `services/users/Cargo.toml` (NEW)
**File**: `services/users/src/lib.rs` (NEW)
**File**: `services/users/src/main.rs` (NEW)
**Changes**: Identical structure to orders — change crate name to `bricks-users`, function to `users_router()`, port to 8082, service name to `bricks-users`.

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

#### 3. Employees Domain Crate
**File**: `services/employees/Cargo.toml` (NEW)
**File**: `services/employees/src/lib.rs` (NEW)
**File**: `services/employees/src/main.rs` (NEW)
**Changes**: Identical structure — crate name `bricks-employees`, function `employees_router()`, port 8083, service name `bricks-employees`.

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

```rust
use axum::Router;

pub fn employees_router() -> Router {
    Router::new()
}
```

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

#### 4. Equipment Domain Crate
**File**: `services/equipment/Cargo.toml` (NEW)
**File**: `services/equipment/src/lib.rs` (NEW)
**File**: `services/equipment/src/main.rs` (NEW)
**Changes**: Identical structure — crate name `bricks-equipment`, function `equipment_router()`, port 8084, service name `bricks-equipment`.

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

```rust
use axum::Router;

pub fn equipment_router() -> Router {
    Router::new()
}
```

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

#### 5. Materials Domain Crate
**File**: `services/materials/Cargo.toml` (NEW)
**File**: `services/materials/src/lib.rs` (NEW)
**File**: `services/materials/src/main.rs` (NEW)
**Changes**: Identical structure — crate name `bricks-materials`, function `materials_router()`, port 8085, service name `bricks-materials`.

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

```rust
use axum::Router;

pub fn materials_router() -> Router {
    Router::new()
}
```

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

#### 6. Notifications Domain Crate
**File**: `services/notifications/Cargo.toml` (NEW)
**File**: `services/notifications/src/lib.rs` (NEW)
**File**: `services/notifications/src/main.rs` (NEW)
**Changes**: Identical structure — crate name `bricks-notifications`, function `notifications_router()`, port 8086, service name `bricks-notifications`.

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

```rust
use axum::Router;

pub fn notifications_router() -> Router {
    Router::new()
}
```

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

### Success Criteria:

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

---

## Phase 4: Docker Compose + Documentation Updates

### Overview

Update `docker-compose.yml` to replace the `api` service with `gateway`. Update `ARCHITECTURE.md` to reflect the new workspace structure. Update `AGENTS.md` to reference the workspace instead of `services/api/`. Remove the old `services/api/` directory.

### Changes Required:

#### 1. Docker Compose
**File**: `docker-compose.yml` (MODIFY)
**Changes**: Replace `api` service with `gateway` service. Update build context to `.` with dockerfile `services/gateway/Dockerfile`. Update `depends_on` in frontend services from `api` to `gateway`.

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

#### 2. Architecture Documentation
**File**: `ARCHITECTURE.md` (MODIFY)
**Changes**: Update directory structure to show workspace layout. Update tech stack to include workspace, thiserror, sqlx, tracing. Add design decisions section.

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

#### 3. AI Agent Guide
**File**: `AGENTS.md` (MODIFY)
**Changes**: Update backend reference from `services/api/` to `services/` workspace. Update commands section. Update "When to Read More" section.

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

#### 4. Remove Old API Directory
**File**: `services/api/` (DELETED)
**Changes**: Remove the entire `services/api/` directory. The workspace structure replaces it entirely.

```bash
rm -rf services/api/
```

### Success Criteria:

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

---

## Testing Strategy

### Automated:
- `cargo build --workspace` — compiles all 8 crates (shared + gateway + 6 domains)
- `cargo test --workspace` — runs tests across all crates
- `cargo build -p bricks-shared` — shared crate compiles independently
- `cargo build -p bricks-gateway` — gateway compiles independently
- `cargo build -p bricks-<domain>` — each domain crate compiles independently (6 crates)
- `docker compose config` — validates docker-compose.yml syntax
- `docker build -f services/gateway/Dockerfile -t bricks-gateway:latest .` — Dockerfile builds

### Manual Testing Steps:
1. Run `cargo run --package bricks-gateway` and verify it starts on port 8080
2. Hit `curl http://localhost:8080/health` and verify `{"status":"ok","service":"bricks-gateway"}`
3. Send SIGTERM to the gateway and verify graceful shutdown message
4. Build and run via Docker: `docker compose up --build gateway` and verify healthcheck passes
5. Verify `services/api/` directory no longer exists
6. Verify `ARCHITECTURE.md` and `AGENTS.md` reference the new workspace structure

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

## Developer Context

## References

- Design: `.rpiv/artifacts/designs/2108da6_rust-microservices-architecture.md`
- Solutions: `.rpiv/artifacts/solutions/rust-microservices-architecture.md`
- Current backend: `services/api/Cargo.toml`, `services/api/src/main.rs`
- Current Docker Compose: `docker-compose.yml`
- Current architecture docs: `ARCHITECTURE.md`, `AGENTS.md`
