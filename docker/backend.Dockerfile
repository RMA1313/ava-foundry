FROM rust:1.89-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/backend/Cargo.toml crates/backend/Cargo.toml
RUN cargo fetch

CMD ["cargo", "run", "--bin", "backend"]
