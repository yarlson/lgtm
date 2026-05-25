FROM node:22-bookworm-slim

ARG CODEX_VERSION="0.133.0"
ARG MISE_VERSION="v2026.5.15"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV MISE_INSTALL_PATH="/usr/local/bin/mise"
ENV MISE_DATA_DIR="/mise"
ENV MISE_CONFIG_DIR="/mise"
ENV MISE_CACHE_DIR="/mise/cache"
ENV MISE_PIN="1"
ENV PATH="/mise/shims:${PATH}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        bubblewrap \
        build-essential \
        ca-certificates \
        curl \
        git \
        pkg-config \
        python3 \
        python3-pip \
        ripgrep \
        sudo \
    && curl https://mise.run | MISE_VERSION="${MISE_VERSION}" sh \
    && npm install -g "@openai/codex@${CODEX_VERSION}" \
    && printf '%s\n' 'export PATH="/mise/shims:${PATH}"' > /etc/profile.d/lgtm-mise-shims.sh \
    && mkdir -p /workspace /root/.codex /mise/cache /mise/shims \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
