FROM node:22-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

RUN corepack enable

CMD ["sh", "-lc", "pnpm --dir packages/frontend install --force --frozen-lockfile && pnpm --dir packages/frontend dev --host 0.0.0.0 --port 5173"]
