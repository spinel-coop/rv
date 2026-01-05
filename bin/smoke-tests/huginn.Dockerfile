# Test rv ci with Huginn
# Expects rv binary in build context

FROM huginn/huginn

# Copy rv binary (passed via build context)
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clear pre-installed gems to test rv ci from scratch
RUN rm -rf vendor/bundle .bundle

# Huginn uses Ruby 3.2
RUN echo "3.2" > .ruby-version

# Install Ruby and run rv ci
# Need to source rv's shell env to set PATH for the installed Ruby
RUN rv ruby install && \
    eval "$(rv shell env bash)" && \
    rv ci -q
