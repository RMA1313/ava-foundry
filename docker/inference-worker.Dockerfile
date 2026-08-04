FROM python:3.11-slim
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

ENV PYTHONPATH=/workspace/services/inference-worker/src

CMD ["python", "-m", "inference_worker"]
