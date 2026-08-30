FROM oven/bun:1.3.14 AS bun-deps
WORKDIR /app

COPY package.json bun.lock ./
COPY apps/web/package.json ./apps/web/package.json
COPY packages/cli/package.json ./packages/cli/package.json

RUN bun install --frozen-lockfile

FROM bun-deps AS web-build

COPY tokens.css ./tokens.css
COPY apps/web ./apps/web

WORKDIR /app/apps/web
RUN bun run build

FROM rust:1.94-bookworm AS rust-build
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY apps/api/Cargo.toml ./apps/api/Cargo.toml
COPY apps/api/src ./apps/api/src
COPY apps/api/migrations ./apps/api/migrations

RUN cargo fetch --locked
RUN cargo build --locked --release -p garmin-fit-extractor-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 10001 garmin-fit \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin garmin-fit \
    && mkdir -p /app/public /data \
    && chown -R 10001:10001 /app /data

COPY --from=rust-build /app/target/release/garmin-fit-extractor-api /usr/local/bin/garmin-fit-extractor-api
COPY --from=web-build /app/apps/web/dist /app/public

USER 10001:10001

ENV GARMIN_FIT_STATIC_DIR=/app/public \
    GARMIN_FIT_DATABASE_URL=sqlite:///data/garmin-fit-extractor.sqlite3

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl --fail http://127.0.0.1:3000/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/garmin-fit-extractor-api"]
