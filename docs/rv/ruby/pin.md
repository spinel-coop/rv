# rv ruby pin [VERSION]

The `pin` command workflow consists of:

- If no argument was given, return the full path to the current Ruby interpreter.
- If `VERSION` was provided, then we:

    1. Parse `VERSION` and validate it as an existing Ruby version, or raise an error.
    2. Check to see if that Ruby version is installed, if not kick off `ruby install VERSION`.
    3. Once the version is installed, check for a `.ruby-version` file in the current project root.
    4. If there is no `.ruby-version` file, create a new file to hold the version number.
    5. Overwrite the contents of the project's `.ruby-version` file with the resolved version.
