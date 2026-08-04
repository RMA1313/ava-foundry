FROM rust:1.89-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

CMD ["cargo", "run", "--bin", "backend"]
