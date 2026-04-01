# frozen_string_literal: true

require "json"

class GemfileSerializer
  def initialize
    @gemfile = {
      global_source: nil,
      deps: [],
      ruby: nil
    }
  end

  def gem(name, *args)
    options = args.last.is_a?(Hash) ? args.pop.dup : {}
    if options.any?
      raise "Dependency options not yet supported"
    end

    requirement = args.empty? ? [">= 0"] : args

    @gemfile[:deps] << { name: name, constraints: requirement }
  end

  def source(url)
    if @gemfile[:global_source]
      raise "Only one global source supported"
    end

    @gemfile[:global_source] = url
  end

  def ruby(*ruby_version)
     options = ruby_version.pop if ruby_version.last.is_a?(Hash)
     ruby_version.flatten!

     if options
       raise "Ruby options not yet supported"
     end

     @gemfile[:ruby] = ruby_version
  end
end

gemfile_path = ARGV[0]

gemfile_serializer = GemfileSerializer.new

gemfile_serializer.instance_eval(File.read(gemfile_path))

puts gemfile_serializer.instance_variable_get(:@gemfile).to_json
