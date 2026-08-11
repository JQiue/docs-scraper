# rust:alpine ships a musl toolchain by default
FROM rust:1.96.1-alpine3.24 AS builder
RUN apk add --no-cache musl-dev
# RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /docs-scraper
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# runtime stage: nothing but the binary
FROM scratch
COPY --from=builder /docs-scraper/target/x86_64-unknown-linux-musl/release/docs-scraper /docs-scraper
ENTRYPOINT ["/docs-scraper"]
