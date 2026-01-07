# Test rv ci with GitLab
# https://gitlab.com/gitlab-org/gitlab

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
# Based on GitLab's source installation docs
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    pkg-config \
    cmake \
    libyaml-dev \
    libre2-dev \
    libffi-dev \
    libxml2-dev \
    libxslt-dev \
    libcurl4-openssl-dev \
    libicu-dev \
    libkrb5-dev \
    libpq-dev \
    libpcre2-dev \
    libgpgme-dev \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone GitLab (shallow clone, it's huge)
RUN git clone --depth 1 https://gitlab.com/gitlab-org/gitlab.git .

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
