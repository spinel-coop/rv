# Test rv ci with Fastlane
# https://github.com/fastlane/fastlane

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    libyaml-dev \
    libffi-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone fastlane
RUN git clone --depth 1 https://github.com/fastlane/fastlane.git .

# Fastlane has no .ruby-version file or Ruby version in Gemfile.lock
# …according to https://docs.fastlane.tools/ - "fastlane supports Ruby versions 2.5 or newer"
RUN echo "3.3" > .ruby-version

# Install Ruby, add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
