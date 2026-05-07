# `rv` plans

All-in-one tooling for Ruby developers.

- Manage Ruby versions, gems, applications, tools, and scripts, effortlessly.
- Handle everything Ruby provided by `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.

## features

- Run any gem command instantly, like `rvx rails new`.
- Run any script, installing all needed gems, like `rv run script.rb`.
- Install gem CLIs with any needed rubies with `rv tool install`.
- Install precompiled Ruby versions in seconds with `rv ruby install`.
- Manage project gems with `rv sync`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

## similar tools

rv combines several functions that have previously been separate tools:

- ruby version manager (like `rvm`, `rbenv`, `chruby`)
- ruby version installer (like `ruby-build`, `ruby-install`)
- gem installer (like `rubygems`)
- dependency installer (like `bundler`)
- project runner (like `npm`, `make`, `rake`)
- package dev & publishing (like `rubygems` & `bundler`)
- running packages (like `gem exec` or `npm exec`)
- installing tools (like `uv tool`, ruby and node lack this today)

## command table of contents

### [Ruby version management](#ruby)

- [x] `rv run CMD`
- [x] [`rv ruby install`](#install)
- [x] `rv ruby list`
- [x] [`rv ruby pin`](#pin)
- [x] `rv ruby dir`
- [x] `rv ruby uninstall`
- [ ] `rv ruby eol`

### Gem CLI tools

- [x] `rvx` / `rv tool run`
- [x] `rv tool install`
- [x] `rv tool uninstall`

### Projects

- [x] `rv clean-install` / `rv ci`
- [ ] [`rv init`](#init)
- [ ] `rv sync`
- [ ] `rv run [TASK]`
- [ ] `rv list [GEM]`
- [ ] `rv upgrade [--all|GEM]`
- [ ] `rv add GEM`
- [ ] `rv remove GEM`
- [ ] `rv tree [GEM]`
- [ ] `rv eol`
- [ ] `rv info GEM`
- [ ] `rv search NAME`
- [ ] `rv new DIR`

  #### Single-file projects (scripts)

- [ ] `rv add --script FILE GEM`
- [ ] `rv remove --script FILE GEM`

### Gems

- [ ] `rv gem NAME`
- [ ] `rv build`
- [ ] `rv publish [SERVER]`

### Shell integration

- [x] `rv shell zsh`
- [x] `rv shell bash`
- [x] `rv shell fish`
- [x] `rv shell nushell`
- [x] `rv shell powershell`

#### Shell integration internal commands

- [x] `rv shell init`
- [x] `rv shell env`
- [x] `rv shell completions`

## interpreter support

- [x] MRI HEAD
- [x] MRI 4.0
- [x] MRI 3.4
- [x] MRI 3.3
- [x] MRI 3.2
- [ ] JRuby 10
- [ ] TruffleRuby 24

### EOL interpreters (maybe)

- [ ] MRI 3.1
- [ ] MRI 3.0
- [ ] MRI 2.7.8
- [ ] MRI 2.5.9
- [ ] MRI 2.3.8
- [ ] MRI 1.9.3
- [ ] MRI 1.8.7
- [ ] MRI 1.2.6
- [ ] MRI 1.0
- [x] MRI 0.49

## supported platforms

- [x] macOS 15+
- [x] Linux (glibc 2.35+)
- [x] Alpine (musl 1.2.5+)
- [x] Windows 11+

## configuration

rv can be configured globally, per user, and per project, as well as via ENV. configuration files use the [KDL](https://kdl.dev) document format.

settings we know we want to support include:

- ruby location(s)
- ruby installation location
- default ruby version

Ruby version can be set by `.ruby-version` (shared with `rvm`, `rbenv`, `chruby`) or `.tool-versions` (shared with `asdf`, and `mise`).

User-wide settings are located in `~/.config/rv/config.kdl` or `~/.rv.kdl`.

Project settings are located in `gem.kdl` at the root of the project directory.

## projects

Projects are libraries, or packages, or applications. A project might be inside a workspace that aggregates together several projects.

A project root may be indicated by a `Gemfile` or a `gem.kdl` config file.

Most `rv` commands (like `add,` `remove,` `install,` and `lock`) are scoped to a project and that project's dependencies. Some commands also interact with the user or global state, like `ruby install`, `tool install`, etc.

## project subtypes

Projects can be one or more of:

- application (like a Rails or Hanami app)
- gem (library for publishing)
- library (structured like a gem but not published)

We may want to provide explicit support for libraries vs gems, although today Bundler treats them as the same.

Applications typically have their own framework generators, so it's unlikely we will need to build any application-specific functionality. On the other hand, gems and libraries are typically generated using `bundle gem` so we will likely want to support generation for that. On the third hand, we may even want to offer an extremely fast templating and generator framework that applications could use instead of Thor.

## workspaces

Workspaces are a group of projects that all have their own dependencies, but are resolved and locked together as a group to ensure that one set of gems will work across all of the packages. In Bundler, this is handled via `path` gems, so we could likely do something similar with a parent `gem.kdl` that points to child gem/library folders.

Supporting workspaces specifically means not just a parent project, but resolving dependencies for each child gem during `rv install` or `rv lock` scoped to the parent workspace, to guarantee every child project uses the same single set of versions resolved for the parent. The effect of commands like `rv add` will remain scoped to a single project (and the closest `gem.kdl`), but they will need to re-resolve the workspace parent `gem.kdl`.

## ruby version file

A ruby version file (named `.ruby-version`) indicates the desired/locked Ruby version for a specific project or workspace. Any project intended to be usable by itself, outside of a workspace context, should declare a Ruby version.

The ruby version can also be provided by a [`.tool-versions` file](https://asdf-vm.com/manage/configuration.html#tool-versions), shared by tools like asdf and mise.

Finally, the ruby version can also be provided by `gem.kdl`, although tools other than `rv` may not be able to read that.

## ruby locations

The main location for Rubies is `$XDG_DATA_HOME/rv/rubies`. We also look for Rubies in locations like `~/.rubies`, `/opt/rubies`, `/opt/homebrew/Cellar/ruby/`, `/usr/local/rubies`, and `/usr/local/Cellar/ruby`.

The default location for `$XDG_DATA_HOME` is `~/.local/share`, so we install rubies into `~/.local/share/rv/rubies` by default.

## `run` vs `exec`

While Bundler invented the idea of `bundle exec` to run commands in the context of a bundle, both node and python have settled on different terminology: `run NAME` is for running commands inside a bundle, while `exec NAME` is for running a package that might not even be installed yet, without adding the package to the current bundle.

## shortcut binaries

In `uv`, `exec` is so common that it has a dedicated binary: `uvx`. This mirrors the `npm` shortcut for `npm exec`, which is named `npx`. That implies we should provide our own shortcut binary named `rvx`.

## what are "tools"?

Tools are executables provided by a package, installed in a fully isolated environment with their own Ruby version and gemset, and run directly via a command placed directly on the PATH. For example, you could run `rv tool install gist` to end up with a `gist` command that works correctly whether you are inside or outside a Gemfile directory, inside or outside `bundle exec`, or even regardless of whether you have Ruby installed at all when you first run `tool install`.

This is roughly the equivalent of `cargo install` or `go install`, but taking care of the additional concerns raised by needing an interpreter. `uv tool install` handles all of this extremely well for Python, but Ruby has never had a tool that does this.

## commands

Notes about the functionality and implementation of each command.

### ruby

The `ruby` subcommand manages ruby versions, using subcommands `install`, `uninstall`, `pin`, and `find`.

#### [install](/docs/rv/ruby/install.md)

The install command downloads a precompiled ruby for the current architecture and operating system, installing it into the rubies directory (which defaults to `~/.local/share/rv/rubies`).

#### [pin](/docs/rv/ruby/pin.md)

Pin with no arguments reports the project's currently chosen version of Ruby.

Pin with a version argument tries to set that version for the current project, validating the version, installing the version if needed, and then writing the version into the current project's `.ruby-version` file.

#### find

The `ruby find` subcommand returns the full path to the currently chosen Ruby interpreter. If passed an argument, it interprets that argument as a version request and prints the full path to a Ruby interpreter that satisfies the version request.

### init

Set up an existing Ruby project to work with `rv`. Create a `gem.kdl` file, import supported settings from `.bundle/config`, import dependencies from `Gemfile`, import package configuration from `*.gemspec`, and print some instructions for anything else that needs to be done manually. After running `rv init`, all the other commands (like `ci`, `run`, `add`, etc) are functional.

### clean-install

The `clean-install` or `ci` command is mainly inspired by `npm ci`, and is functionally very similar to `bundle install --frozen`. It installs dependencies as described by the lockfile, and does not interact with the Gemfile.

### run

The `run` command executes commands and files provided by the current project or filesystem. Contrast to `exec`, below, which executes commands provided by installing gems. There are several sources of commands for `run`: 1) the $PATH, 2) your project, 3) a file

1. Anything in your `$PATH` can be run by `rv run`, including `ruby`! In practice, this sets up a Ruby environment and then runs your command. For example `rv run --ruby 3.4.2 -- ruby` will run ruby 3.4.2 regardless of which ruby version is currently activated by the environment and the current project. Similarly, if you run `rv run bash` and then try to run `ruby`, you will get the Ruby version configured by rv.

2. Your project can provide commands for `run` in two ways: 1) by defining named commands in the `gem.kdl` configuration file, like `npm run` with `package.json`, or 2) declaring your project provides binaries, like the `gist` gem provides the `gist` command. If you are working on the `gist` gem, you can `rv run gist` to run the current project's command.

3. You can write a Ruby script and then run it with `rv run script.rb`. Scripts can optionally contain their own required ruby versions and rubygems dependencies, as a magic comment with the same structure as `gem.kdl`. If the script has configuration comments setting a required ruby version or depending on gems, rv will install that ruby version and those gems and then run thes cript. If the script does not declare any Ruby or gem dependencies, rv will simply ensure a Ruby is installed and use it to run the script.

### shell

The `shell` subcommand handles integration with the user's shell, including automatic ruby version switching and completions for rv commands.

#### bash / zsh / fish / nushell / powershell

Passing the name of a shell, as in `rv shell zsh`, prints out instructions for configuring that shell with rv's ruby version management and CLI completions.

#### init (hidden)

The `init` command prints out shellscript that sets up automatic version switching, and is intended to be used like `eval "$(rv shell init zsh)"` to set up zsh to then automatically run `eval "$(rv shell env)"` every time the user changes directories, which provides automatic version switching.

#### env (hidden)

The `env` command prints out the env vars that need to be set for the currently-desired ruby version, like `RUBY_VERSION` and `PATH`. The output is expected to be `eval`ed by the shell to change the installation that will run as `ruby`.

#### completions (hidden)

The `completions` command prints out shell-specific output that can be `eval`ed to set up tab-completion for subcommands and arguments to commands.

### tool

The tool subcommand manages binaries available on the PATH, ensuring that a usable Ruby is installed, the gem and all of its dependencies are installed, and a binary is created and put somewhere in the PATH. The binary needs to ignore the currently chosen ruby version, the current bundle environment, and anything else necessary to ensure that when it is invoked it will run completely independently.

#### tool run / rvx

The `tool run` command runs commands provided by gems. Contrast with `run`, above, which executes commands provided by the current project or filesystem.

Just like `npm exec`, `uvx`, and `gem exec`, `rv exec` will: find a package with the given name, install it, and run the executable inside that package with the same name. For example, `rv exec rails new .` will install ruby if needed, install rails if needed, and then run `rails new .`.

Similar to npm and uv, `rvx rails@latest new .` will check for the newest version of Rails and make sure that is what gets run. Without the `@latest` included at the end of the package name, `exec` will prioritize speed and run an already-installed rails if it exists.
