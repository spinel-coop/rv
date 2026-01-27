# -*- encoding: utf-8 -*-
# stub: cssminify 1.0.2 ruby lib

Gem::Specification.new do |s|
  s.name = "cssminify".freeze
  s.version = "1.0.2".freeze

  s.required_rubygems_version = Gem::Requirement.new(">= 0".freeze) if s.respond_to? :required_rubygems_version=
  s.require_paths = ["lib".freeze]
  s.authors = ["Matthias Siegel".freeze]
  s.date = "2012-06-30"
  s.description = "    The CSSminify gem provides CSS compression using YUI compressor. Instead of wrapping around the Java or Javascript version of YUI compressor it uses a native Ruby port of the CSS engine. Therefore this gem has no dependencies.\n".freeze
  s.email = "matthias.siegel@gmail.com".freeze
  s.extra_rdoc_files = ["CHANGES.md".freeze, "LICENSE.md".freeze, "README.md".freeze]
  s.files = ["CHANGES.md".freeze, "LICENSE.md".freeze, "README.md".freeze]
  s.homepage = "https://github.com/matthiassiegel/cssminify".freeze
  s.licenses = ["MIT".freeze]
  s.rubygems_version = "1.8.10".freeze
  s.summary = "CSS minification with YUI compressor, but as native Ruby port.".freeze

  s.installed_by_version = "4.0.3".freeze

  s.specification_version = 3

  s.add_development_dependency(%q<rspec>.freeze, ["~> 2.7".freeze])
end
