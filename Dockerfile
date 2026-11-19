# ForgeHub — multi-stage build: web app + Rust server, slim runtime.
# Build:  docker build -t forgehub .
# Run:    docker run -p 8080:8080 -e FORGEHUB_SECRET_KEY=<48+ random chars> -v forgehub-data:/data forgehub

FROM node:24-slim AS web
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --legacy-peer-deps
COPY web/ ./
RUN npm run build

FROM rust:1-slim-bookworm AS server
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY docs ./docs
COPY README.md LICENSE ./
# Stamp the image with its source revision (Step 21):
# /api/v1/version reports it; the build also embeds the
# checkout's own git sha when built from a clone.
ARG GIT_SHA=""
ENV FORGEHUB_GIT_SHA=$GIT_SHA
RUN cargo build --release -p forge-server

FROM debian:bookworm-slim AS runtime
# The build context has no .git, so the revision stamp comes
# from this build argument (compose passes GIT_SHA through).
ARG GIT_SHA=""
ENV FORGEHUB_GIT_SHA=$GIT_SHA
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --create-home --uid 10001 forgehub \
 && mkdir -p /data && chown forgehub:forgehub /data
COPY --from=server /build/target/release/forge-server /usr/local/bin/forge-server
COPY --from=web /build/web/dist /srv/web
USER forgehub
ENV FORGEHUB_ENV=prod \
    FORGEHUB_BIND=0.0.0.0:8080 \
    FORGEHUB_DATABASE_URL=sqlite:///data/forgehub.db \
    FORGEHUB_WEB_DIR=/srv/web
EXPOSE 8080
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/api/v1/health || exit 1
ENTRYPOINT ["forge-server"]
