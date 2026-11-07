# syntax=docker/dockerfile:1

# ---- build ---------------------------------------------------------------
FROM rust:1.97-slim-bookworm AS build
WORKDIR /app
# Use the checked-in .sqlx query data: no database needed to compile.
ENV SQLX_OFFLINE=true
COPY . .
# Cache the registry and target dir between builds (BuildKit cache mounts).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --bin slotbook \
 && cp target/release/slotbook /usr/local/bin/slotbook

# ---- run -----------------------------------------------------------------
# Distroless: glibc and CA certificates, no shell, no package manager.
# Same Debian release as the build image, so glibc versions match.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /usr/local/bin/slotbook /app/slotbook
COPY static /app/static
ENV BIND_ADDR=0.0.0.0:8080 \
    STATIC_DIR=/app/static \
    LOG_FORMAT=json \
    RUST_LOG=slotbook=info,tower_http=info
EXPOSE 8080
# Runs as the image's non-root user (uid 65532).
USER nonroot
ENTRYPOINT ["/app/slotbook"]
