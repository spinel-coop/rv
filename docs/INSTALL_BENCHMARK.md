# Install Benchmark

Here's the install time difference running on a base M5 MacBook Pro in Low Power Mode and High Power Mode:

| | seconds | times slower |
| ---: | ---: | ---: |
| rv | 2.479 | 1.0x |
| ruby-build (High Power) | 89.611 | 36.14x |
| ruby-build (Low Power) | 159.752 | 64.44x |

We happen to be using `rbenv` here, but `chruby`, `mise` and other tools all use `ruby-build` under the hood to compile Ruby during install.

`rv` install time is constrained by network speed so your mileage will vary. This was run from Copenhagen, Denmark on a reasonably fast connection.

## rv install time

```bash
time rv ruby install 3.4.7
Downloaded https://github.com/spinel-coop/rv-ruby/releases/latest/download/ruby-3.4.7.arm64_sonoma.tar.gz to ~/.cache/rv/ruby-v0/tarballs/8758fed525bd3750.tar.gz
Installed Ruby version ruby-3.4.7 to ~/.local/share/rv/rubies

real 0m2.479s
user 0m0.362s
sys  0m0.551s
```

## rbenv + ruby-build install in High Power Mode

```bash
time rbenv install 3.4.7
ruby-build: using openssl@3 from homebrew
==> Downloading ruby-3.4.7.tar.gz...
-> curl -q -fL -o ruby-3.4.7.tar.gz https://cache.ruby-lang.org/pub/ruby/3.4/ruby-3.4.7.tar.gz
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100 22.1M  100 22.1M    0     0  11.9M      0  0:00:01  0:00:01 --:--:-- 11.9M
==> Installing ruby-3.4.7...
ruby-build: using libyaml from homebrew
ruby-build: using gmp from homebrew
-> ./configure "--prefix=$HOME/.rbenv/versions/3.4.7" --with-openssl-dir=/opt/homebrew/opt/openssl@3 --enable-shared --with-libyaml-dir=/opt/homebrew/opt/libyaml --with-gmp-dir=/opt/homebrew/opt/gmp --with-ext=openssl,psych,+
-> make -j 10
-> make install
==> Installed ruby-3.4.7 to ~/.rbenv/versions/3.4.7

real 1m29.611s
user 2m54.163s
sys  0m57.157s
```

## rbenv + ruby-build install in Low Power Mode

```bash
time rbenv install 3.4.7
ruby-build: using openssl@3 from homebrew
==> Downloading ruby-3.4.7.tar.gz...
-> curl -q -fL -o ruby-3.4.7.tar.gz https://cache.ruby-lang.org/pub/ruby/3.4/ruby-3.4.7.tar.gz
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100 22.1M  100 22.1M    0     0  6721k      0  0:00:03  0:00:03 --:--:-- 6719k
==> Installing ruby-3.4.7...
ruby-build: using libyaml from homebrew
ruby-build: using gmp from homebrew
-> ./configure "--prefix=$HOME/.rbenv/versions/3.4.7" --with-openssl-dir=/opt/homebrew/opt/openssl@3 --enable-shared --with-libyaml-dir=/opt/homebrew/opt/libyaml --with-gmp-dir=/opt/homebrew/opt/gmp --with-ext=openssl,psych,+
-> make -j 10
-> make install
==> Installed ruby-3.4.7 to ~/.rbenv/versions/3.4.7

real 2m39.752s
user 4m41.813s
sys  1m35.644s
```
