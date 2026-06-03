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
    └── api/       # Rust backend service
```

## Tech Stack
- **Frontends:** Vanilla TypeScript, Vite (build/dev), Vitest (unit tests), Playwright (E2E), ESLint, Prettier
- **Backend:** Rust, axum (web framework), Cargo (build tool)
- **Orchestration:** Docker Compose
- **No monorepo tooling:** Each project is self-contained with its own dependencies and configs

## Design Decisions
- No shared configuration packages — each project maintains its own ESLint, Prettier, and TypeScript configs
- No build orchestration tools (Turborepo, Nx) — projects are independent
- AGENTS.md at root for AI agent routing with progressive disclosure
- Docker Compose for service orchestration
