# Baseline: Traditional approach using official Ruby Docker image
# https://github.com/lobsters/lobsters

FROM ruby:4.0-slim-bookworm

# Install dependencies for native gems
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    curl \
    ca-certificates \
    pkg-config \
    libyaml-dev \
    libmariadb-dev \
    libsqlite3-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust (needed for commonmarker gem)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Clone Lobsters
RUN git clone --depth 1 https://github.com/lobsters/lobsters.git .

# Install gems
RUN bundle install
