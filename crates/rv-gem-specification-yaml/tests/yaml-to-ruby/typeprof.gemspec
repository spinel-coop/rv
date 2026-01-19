# -*- encoding: utf-8 -*-
# stub: typeprof 0.31.1 ruby lib

Gem::Specification.new do |s|
  s.name = "typeprof".freeze
  s.version = "0.31.1".freeze

  s.required_rubygems_version = Gem::Requirement.new(">= 0".freeze) if s.respond_to? :required_rubygems_version=
  s.metadata = { "homepage_uri" => "https://github.com/ruby/typeprof", "source_code_uri" => "https://github.com/ruby/typeprof" } if s.respond_to? :metadata=
  s.require_paths = ["lib".freeze]
  s.authors = ["Yusuke Endoh".freeze]
  s.date = "2026-01-13"
  s.description = "TypeProf performs a type analysis of non-annotated Ruby code.\n\nIt abstractly executes input Ruby code in a level of types instead of values, gathers what types are passed to and returned by methods, and prints the analysis result in RBS format, a standard type description format for Ruby 3.0.\n".freeze
  s.email = ["mame@ruby-lang.org".freeze]
  s.executables = ["typeprof".freeze]
  s.files = ["bin/typeprof".freeze]
  s.homepage = "https://github.com/ruby/typeprof".freeze
  s.licenses = ["MIT".freeze]
  s.required_ruby_version = Gem::Requirement.new(">= 3.3".freeze)
  s.rubygems_version = "4.0.3".freeze
  s.summary = "TypeProf is a type analysis tool for Ruby code based on abstract interpretation".freeze

  s.installed_by_version = "4.0.3".freeze

  s.specification_version = 4

  s.add_runtime_dependency(%q<prism>.freeze, [">= 1.4.0".freeze])
  s.add_runtime_dependency(%q<rbs>.freeze, [">= 3.6.0".freeze])
end
