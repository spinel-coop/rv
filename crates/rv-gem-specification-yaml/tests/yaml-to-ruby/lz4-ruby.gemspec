# -*- encoding: utf-8 -*-
# stub: lz4-ruby 0.3.3 ruby lib
# stub: ext/lz4ruby/extconf.rb

Gem::Specification.new do |s|
  s.name = "lz4-ruby".freeze
  s.version = "0.3.3".freeze

  s.required_rubygems_version = Gem::Requirement.new(">= 0".freeze) if s.respond_to? :required_rubygems_version=
  s.require_paths = ["lib".freeze]
  s.authors = ["KOMIYA Atsushi".freeze]
  s.date = "2014-07-10"
  s.description = "Ruby bindings for LZ4. LZ4 is a very fast lossless compression algorithm.".freeze
  s.email = ["komiya.atsushi@gmail.com".freeze]
  s.extensions = ["ext/lz4ruby/extconf.rb".freeze]
  s.homepage = "http://github.com/komiya-atsushi/lz4-ruby".freeze
  s.licenses = ["MIT".freeze]
  s.required_ruby_version = Gem::Requirement.new(">= 1.9".freeze)
  s.rubygems_version = "2.0.14".freeze
  s.summary = "Ruby bindings for LZ4 (Extremely Fast Compression algorithm).".freeze

  s.installed_by_version = "4.0.3".freeze

  s.specification_version = 4

  s.add_development_dependency(%q<rspec>.freeze, [">= 0".freeze])
  s.add_development_dependency(%q<rdoc>.freeze, ["~> 3.12".freeze])
  s.add_development_dependency(%q<bundler>.freeze, [">= 0".freeze])
  s.add_development_dependency(%q<jeweler>.freeze, ["~> 1.8.3".freeze])
  s.add_development_dependency(%q<rake-compiler>.freeze, [">= 0".freeze])
end
