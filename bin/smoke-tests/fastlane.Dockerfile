# Test rv ci with Fastlane
# Expects rv binary in build context

FROM debian:bookworm-slim

# Install dependencies for rv and native extensions
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    git \
    libssl-dev \
    libreadline-dev \
    zlib1g-dev \
    libyaml-dev \
    libffi-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary (passed via build context)
COPY rv /usr/local/bin/rv

WORKDIR /root

# Clone fastlane
RUN git clone --depth 1 https://github.com/fastlane/fastlane.git

WORKDIR /root/fastlane

# Fastlane requires Ruby >= 2.6 but has no .ruby-version file
RUN echo "3.3" > .ruby-version

# Install Ruby and run rv ci
RUN rv ruby install && \
    eval "$(rv shell env bash)" && \
    rv ci -q
