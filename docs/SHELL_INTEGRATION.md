# Shell integration

rv integrates with [zsh](#zsh), [bash](#bash), [fish](#fish), and [nushell](#nushell).

Run `rv shell <zsh|bash|fish|nu>` to get instructions to set up `rv` to
automatically read any `.ruby-version` or `.tool-versions` file right before
every shell command. If necessary, the shell integration will change your
PATH, GEM_HOME, and other env vars as needed to ensure that the `ruby` command
will run the expected version of ruby.
