# rv ruby install VERSION

The install workflow consists of:

1. Parse VERSION as a version request
1. Validate that the version request is a version that exists
1. Check if that version is installed, and exit if it is
1. Use the version request, architecture, and OS to construct a tarball filename
1. Check if the tarball already exists in the rv cache directory
1. If the file doesn't exist, construct a URL and download the file from the URL
1. Expand the tarball into the first rubies install directory
1. Test that the install worked by running the ruby interpreter
1. Report success
