# Dockerfile for Huginn smoke test
# Includes system dependencies needed for native Ruby extensions

FROM debian:bookworm-slim

# Install Ruby build dependencies and common native extension requirements
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Ruby build essentials
    build-essential \
    curl \
    ca-certificates \
    git \
    # Ruby dependencies
    libssl-dev \
    libreadline-dev \
    zlib1g-dev \
    libyaml-dev \
    libffi-dev \
    # Native extension dependencies (nokogiri, mysql2, pg, etc.)
    libxml2-dev \
    libxslt1-dev \
    libpq-dev \
    default-libmysqlclient-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust for building rv
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

# Entry point will be provided by the smoke test script
CMD ["/bin/bash"]
