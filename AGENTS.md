# Bricks Monorepo — AI Agent Guide

## Repo Structure
- `apps/orders/` — Public order creation frontend (vanilla TS, Vite, Vitest, Playwright)
- `apps/admin/` — Brick management admin frontend (vanilla TS, Vite, Vitest, Playwright)
- `services/api/` — Rust backend service (axum, Cargo)

## Commands
- `npm install` — Install dependencies for current directory
- `docker compose up` — Start all services
- `docker compose up --build orders` — Start orders service only

## Shared Rules
- Each project is self-contained — no shared configs between projects
- Frontend apps: vanilla TypeScript, no framework
- Backend services: Rust with Cargo
- Run linting and tests before committing
- Use Docker Compose for service orchestration

## When to Read More
- For orders app details → read `apps/orders/README.md`
- For admin app details → read `apps/admin/README.md`
- For backend details → read `services/api/README.md`
- For architecture decisions → read `ARCHITECTURE.md`
