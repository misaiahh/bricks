# Bricks Monorepo

Brick-making business application monorepo.

## Projects
- [apps/orders](apps/orders/) — Public order creation frontend
- [apps/admin](apps/admin/) — Brick management admin frontend
- [services/](services/) — Rust backend microservices workspace (see [plan](.rpiv/artifacts/plans/2026-06-03_14-00-00_rust-microservices-architecture.md) for implementation details)

## Getting Started
1. Ensure Docker is installed
2. Run `docker compose up` to start all services
3. See individual project READMEs for development instructions

## Architecture
See [ARCHITECTURE.md](ARCHITECTURE.md) for technical decisions.

## Backend Implementation
The Rust backend is being migrated from a single API to a microservices workspace. See the [implementation plan](.rpiv/artifacts/plans/2026-06-03_14-00-00_rust-microservices-architecture.md) for the phased approach.
