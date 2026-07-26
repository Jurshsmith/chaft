#!/usr/bin/env ruby
# frozen_string_literal: true

# Parse YAML with Psych and reject duplicate mapping keys at every depth.

require "psych"

def key_label(node)
  return node.value if node.is_a?(Psych::Nodes::Scalar)

  "<#{node.class.name.split("::").last}>"
end

def inspect_node(node, path, errors)
  case node
  when Psych::Nodes::Mapping
    seen = {}
    node.children.each_slice(2) do |key, value|
      label = key_label(key)
      location = "#{path}:#{key.start_line + 1}"
      if seen.key?(label)
        errors << "#{location}: duplicate YAML key #{label.inspect} " \
                  "(first declared at line #{seen[label]})"
      else
        seen[label] = key.start_line + 1
      end
      inspect_node(value, path, errors)
    end
  when Psych::Nodes::Sequence, Psych::Nodes::Stream, Psych::Nodes::Document
    node.children.each { |child| inspect_node(child, path, errors) }
  end
end

if ARGV.empty?
  warn "usage: #{$PROGRAM_NAME} <yaml-file>..."
  exit 2
end

errors = []
ARGV.each do |path|
  begin
    tree = Psych.parse_stream(File.read(path, encoding: "UTF-8"), filename: path)
  rescue Psych::SyntaxError, SystemCallError => error
    errors << error.message
    next
  end
  inspect_node(tree, path, errors)
end

unless errors.empty?
  errors.each { |error| warn error }
  exit 1
end

puts "strict YAML parse passed: #{ARGV.length} file(s)"
