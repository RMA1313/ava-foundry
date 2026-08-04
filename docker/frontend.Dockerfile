FROM node:22-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

RUN corepack enable

COPY packages/frontend/package.json packages/frontend/pnpm-lock.yaml packages/frontend/pnpm-workspace.yaml packages/frontend/
RUN pnpm --dir packages/frontend install --frozen-lockfile

CMD ["pnpm", "--dir", "packages/frontend", "dev", "--host", "0.0.0.0", "--port", "5173"]
