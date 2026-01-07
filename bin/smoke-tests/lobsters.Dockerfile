# Test rv ci with Lobsters
# https://github.com/lobsters/lobsters

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    libyaml-dev \
    libmariadb-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Lobsters
RUN git clone --depth 1 https://github.com/lobsters/lobsters.git .

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
