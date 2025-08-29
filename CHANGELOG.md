# rv 0.1.1

* Dual license MIT / Apache-2
* turn up LTO for smaller releases
* Fix `rv ruby pin` if not run in a project_dir
* Fix `bin/setup` in non-root situations under Ubuntu
* replace openssl with rustls
* Added bash to supported shells

# rv 0.1.0

First release. Supports macOS 14+, Ubuntu 24.04+, Ruby 3.4.1+, and zsh.

- `rv ruby` command group
  - `rv ruby list` command to show installed rubies
  - `rv ruby install` to install precompiled rubies
  - `rv ruby run` to run a specific ruby version regardless of `.ruby-version`
- `rv shell` command group
  - `rv shell init` command to set up automatic ruby switching that respects `.ruby-version` files
