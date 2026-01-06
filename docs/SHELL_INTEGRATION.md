# Shell integration

rv integrates with zsh, bash, fish, and nushell.

Run `rv shell <zsh|bash|fish|nu>` to get instructions to set up `rv` integration
with your shell. After this one-time setup, `rv` will automatically use
`.ruby-version` or `.tool-versions` files to give you the requested Ruby.

If necessary, the shell integration will change your PATH, GEM_HOME, and other
env vars as needed to ensure that the `ruby` command will run the expected
version of ruby.

View or update the version of Ruby used in a project by running `rv ruby pin
VERSION`.
