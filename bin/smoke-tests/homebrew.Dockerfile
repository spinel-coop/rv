# Test rv ci with Homebrew
# https://github.com/Homebrew/brew

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
# Homebrew's gems include profilers (ruby-prof, stackprof) and pycall
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    libffi-dev \
    libyaml-dev \
    python3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Homebrew
RUN git clone --depth 1 https://github.com/Homebrew/brew.git .

# Homebrew's Gemfile is in Library/Homebrew/
WORKDIR /app/Library/Homebrew

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
