FROM python:3.11-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

COPY services/inference-worker/pyproject.toml services/inference-worker/pyproject.toml
COPY services/inference-worker/src services/inference-worker/src
RUN pip install --no-cache-dir -e /workspace/services/inference-worker pytest

ENV PYTHONPATH=/workspace/services/inference-worker/src

CMD ["python", "-m", "inference_worker"]
