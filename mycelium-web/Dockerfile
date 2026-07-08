# Multi-stage: build the myc CLI (Rust), build the React frontend (Bun),
# then a slim runtime that serves the API + static assets.

# --- stage 1: myc CLI --------------------------------------------------------
FROM rust:1.82-slim AS myc-build
WORKDIR /src
# Build context is the repo root (see docker-compose build.context: ..)
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin myc

# --- stage 2: frontend -------------------------------------------------------
FROM oven/bun:1 AS web-build
WORKDIR /web
COPY mycelium-web/web/package.json mycelium-web/web/bun.lock* ./
RUN bun install --frozen-lockfile || bun install
COPY mycelium-web/web ./
RUN bun run build

# --- stage 3: runtime --------------------------------------------------------
FROM oven/bun:1-slim AS runtime
WORKDIR /app

# server deps
COPY mycelium-web/server/package.json mycelium-web/server/bun.lock* ./
RUN bun install --frozen-lockfile || bun install

# server source + built frontend + myc binary
COPY mycelium-web/server ./
COPY --from=web-build /web/dist ./public
COPY --from=myc-build /src/target/release/myc /usr/local/bin/myc

ENV MYC_BIN=/usr/local/bin/myc
ENV MYC_PROJECT_DIR=/data
ENV PORT=8787
EXPOSE 8787

# /data holds the mounted .mycelium project
VOLUME ["/data"]

CMD ["bun", "run", "index.ts"]
