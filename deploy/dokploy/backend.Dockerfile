ARG RUST_VERSION=1.95.0

FROM rust:${RUST_VERSION}-bookworm AS builder
WORKDIR /app/backend

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/data/platform_registry_snapshot.json ./data/platform_registry_snapshot.json
COPY frontend/static/brand /app/frontend/static/brand

RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates curl fonts-dejavu-core \
	&& rm -rf /var/lib/apt/lists/* \
	&& useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin firewatch \
	&& mkdir -p /app /data /blobs \
	&& chown -R firewatch:firewatch /app /data /blobs

WORKDIR /app
ENV HTTP_BIND=0.0.0.0:8080
ENV DATABASE_URL=sqlite:///data/indexer.db
ENV BLOB_STORE_DIR=/blobs

COPY --from=builder /app/backend/target/release/firewatch-backend /usr/local/bin/firewatch-backend

USER firewatch
EXPOSE 8080
VOLUME ["/data", "/blobs"]

ENTRYPOINT ["firewatch-backend"]
