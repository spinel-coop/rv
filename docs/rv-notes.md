# rv notes

## functionality

rv combines several things that have previously been separate tools:

- ruby version manager (like rvm, rbenv, chruby)
- ruby version installer (like ruby-build, ruby-install)
- gem installer (like rubygems)
- dependency installer (like bundler)
- project runner (like bundler, npm, make, rake)
- package dev & publishing (like rubygems+bundler)
- running packages (like gemx or npx)
- installing tools (like `uv tool`, ruby and node lack this today)

## configuration

rv can be configured globally, per user, and per project, as well as via ENV.

settings we know we want to support include:

- ruby install location (default /opt/rubies, ~/.rubies)
- default ruby version

## projects

Projects are libraries, or packages, or applications. The required file indicating a project root is a `Gemfile`, but a project with a `Gemfile` might still be inside a workspace that aggregates together several projects.

A project root may be indicated by a `Gemfile`, an `rv.toml` config file, a `.git` directory, a `.jj` directory, or other files to be added in the future.

Most `rv` commands (like `add,` `remove,` `install,` and `lock`) are scoped to a project and that project's dependencies. Some commands also interact with the user or global state, like `ruby install`, `tool install`, etc.

## project subtypes

Projects can be one of:

- application (like a Rails or Hanami app)
- gem (published library)
- library (structured like a gem but not published)

Applications typically have their own framework generators, so it's unlikely we will need to build any application-specific functionality. On the other hand, gems and libraries are typically generated using `bundle gem`.

We may want to provide explicit support for libraries vs gems. If we get super ambitious, we may want to offer an extremely faster generator framework that applications could use instead of Thor.

## workspaces

Workspaces are a group of projects that all have their own dependencies, but are resolved and locked together as a group to ensure that one set of gems will work across all of the packages.

Supporting workspaces means commands like `rv add` are scoped to a single project (and the closest `Gemfile`), while resolving dependencies for `rv install` or `rv lock` should be scoped to the parent workspace (if one exists), the parent Gemfile, and all projects inside the parent Gemfile.

## ruby version file

A ruby version file (named `.ruby-version`) indicates the desired/locked Ruby version for a specific project or workspace. Any project intended to be usable by itself, outside of a workspace context, should declare a desired Ruby version.

## ruby locations

By default, we look for rubies in ~/.rubies, /opt/rubies, /opt/homebrew/bin/ruby, /usr/bin/ruby, and /usr/local/bin/ruby.

By default, we install rubies into ~/.rubies.

## `run` vs `exec`

While Bundler invented the idea of `bundle exec` to run commands in the context of a bundle, both node and python have settled on different terminology: `run NAME` is for running commands inside a bundle, while `exec NAME` is for running a package that might not even be installed yet, whether inside an application or not, without adding the package to the current bundle.

## shortcut binaries

In `uv`, the `run` and `exec` subcommands are both so common that they have their own dedicated binaries: `uvr` and `uxv`. This mirrors the `npm` shortcut for `npm exec`, which is named `npx`. That implies we should provide shortcut binaries named `rvr` and `rvx`.

## what are "tools"?

Tools are executables provided by a package, installed in a fully isolated environment with their own Ruby version and gemset, and run directly via a command placed directly on the PATH. For example, you could run `rv tool install gist` to end up with a `gist` command that works correctly whether you are inside or outside a Gemfile directory, inside or outside `bundle exec`, or even regardless of whether you have Ruby installed at all when you first run `tool install`.

This is roughly the equivalent of `cargo install` or `go install`, but taking care of the additional concerns raised by needing an interpreter. `uv tool install` handles all of this extremely well for Python, but Ruby has never had a tool that does this.

## subcommands

Notes about the functionality and implementation of each command.

### ruby

The `ruby` subcommand manages ruby versions, using subcommands `install`, `uninstall`, `pin`, and `find`.

#### install

The install command downloads a precompiled ruby for the current architecture and operating system, installing it into the rubies directory (which defaults to ~/.rubies).

#### [pin](/docs/rv/ruby/pin.md)

Pin with no arguments reports the project's currently chosen version of Ruby.

Pin with a version argument tries to set that version for the current project, validating the version, installing the version if needed, and then writing the version into the current project's `.ruby-version` file.

#### find

The `ruby find` subcommand returns the full path to the currently chosen Ruby interpreter. If passed an argument, it interprets that argument as a version request and prints the full path to a Ruby interpreter that satisfies the version request.

### tool

The tool subcommand manages binaries available on the PATH, ensuring that a usable Ruby is installed, the gem and all of its dependencies are installed, and a binary is created and put somewhere in the PATH. The binary needs to ignore the currently chosen ruby version, the current bundle environment, and anything else necessary to ensure that when it is invoked it will run completely independently.

## open questions

1. Should configuration use KDL instead of TOML?
    TOML is more popular, has wider ecosystem support, and has more packages and tooling already available.
    On the other hand, KDL handles nesting dramatically better, composes multiple files together better, and TOML's creator [has some issues](https://en.wikipedia.org/wiki/Tom_Preston-Werner#Resignation_from_GitHub).

2. Should we build support for an `rbproject.toml` or similar file to configure projects? It could potentially replace `Gemfile`, `.gemspec`, `.ruby-versions`, `.bundle/config`, `Rakefile`, `.rubocop.yaml`, and any other dependency, package, linter, or script configurations. It would be nice to end the reign of the Filefile, and it would provide a place for arbitrary machine-formatted data that tooling could use. It would also provide a location to configure future multi-project workspaces.

3. Should we match the python and node naming convention and switch to `rv run NAME` to run a command from inside the bundle, reserving `rv exec NAME` for installing and running commands from gems that are not in the bundle? I (André) think we probably should do this, because it has become a standard in tooling for two other languages that are both numerically more popular than Ruby, but I might be missing something.
