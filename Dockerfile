# rust:alpine ships a musl toolchain by default, so the resulting
FROM rust:1.96.1-alpine3.24 AS builder
RUN apk add --no-cache musl-dev
# RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# ---- runtime stage: nothing but the binary ----
FROM scratch
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/docs-scraper /app/docs-scraper
ENTRYPOINT ["/app/docs-scraper"]
