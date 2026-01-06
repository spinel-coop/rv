# Test rv ci with Huginn
# Uses Huginn's own prepare script for dependencies

FROM rubylang/ruby:3.2-jammy

# Clone Huginn and run their prepare script for dependencies
RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN git clone --depth 1 https://github.com/huginn/huginn.git .
RUN cp -r docker/scripts /scripts && chmod +x /scripts/*
RUN /scripts/prepare

# Copy rv binary
COPY rv /usr/local/bin/rv

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
