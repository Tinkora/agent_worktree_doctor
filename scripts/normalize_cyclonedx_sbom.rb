#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "pathname"
require "tempfile"

LOCAL_URI = %r{(?:path\+)?file://}.freeze

def component_tree(component)
  [component, *component.fetch("components", []).flat_map { |child| component_tree(child) }]
end

def all_strings(value, &block)
  case value
  when Hash
    value.each_value { |child| all_strings(child, &block) }
  when Array
    value.each { |child| all_strings(child, &block) }
  when String
    yield value
  end
end

def target_fragment(component, index)
  fragment = component["purl"].to_s.split("#", 2)[1]
  return fragment if fragment&.match?(%r{\A[0-9A-Za-z._~!$&'()*+,;=:@%/-]+\z}) && !fragment.split("/").include?("..")

  type = component.fetch("type", "target").gsub(/[^0-9A-Za-z._-]/, "_")
  name = component.fetch("name", "component").gsub(/[^0-9A-Za-z._-]/, "_")
  "target/#{type}-#{name}-#{index}"
end

path = ARGV.fetch(0) { abort("usage: normalize_cyclonedx_sbom.rb SBOM.cdx.json") }
input = Pathname(path)
abort("SBOM input must be a regular file") unless input.file? && !input.symlink?

document = JSON.parse(input.read(encoding: "UTF-8"))
abort("input is not a CycloneDX SBOM") unless document["bomFormat"] == "CycloneDX"
root = document.dig("metadata", "component")
abort("CycloneDX metadata component is missing") unless root.is_a?(Hash)

name = root["name"]
version = root["version"]
valid_identity = name.is_a?(String) && name.match?(/\A[a-zA-Z0-9_-]+\z/) &&
  version.is_a?(String) && version.match?(/\A[0-9A-Za-z.+-]+\z/)
abort("CycloneDX root package identity is invalid") unless valid_identity

root_ref = "pkg:cargo/#{name}@#{version}"
ref_map = {}
old_root_ref = root["bom-ref"]
ref_map[old_root_ref] = root_ref if old_root_ref.is_a?(String)
root["bom-ref"] = root_ref
root["purl"] = root_ref

components = [root, *document.fetch("components", [])].flat_map { |component| component_tree(component) }
components.drop(1).each_with_index do |component, index|
  next unless component.is_a?(Hash)
  next unless [component["bom-ref"], component["purl"]].compact.any? { |value| value.match?(LOCAL_URI) }

  normalized_ref = "#{root_ref}##{target_fragment(component, index)}"
  old_ref = component["bom-ref"]
  ref_map[old_ref] = normalized_ref if old_ref.is_a?(String)
  component["bom-ref"] = normalized_ref
  component["purl"] = normalized_ref
end

document.fetch("dependencies", []).each do |dependency|
  dependency["ref"] = ref_map.fetch(dependency["ref"], dependency["ref"])
  dependency["dependsOn"] = dependency.fetch("dependsOn", []).map { |ref| ref_map.fetch(ref, ref) }
end

all_strings(document) do |value|
  abort("normalized SBOM still contains a local file URI") if value.match?(LOCAL_URI)
end

component_refs = components.map { |component| component["bom-ref"] }
invalid_refs = component_refs.any? { |ref| !ref.is_a?(String) || ref.empty? }
abort("normalized SBOM has a missing or duplicate component ref") if invalid_refs || component_refs.uniq.length != component_refs.length

dependency_refs = document.fetch("dependencies", []).flat_map do |dependency|
  [dependency["ref"], *dependency.fetch("dependsOn", [])]
end
abort("normalized SBOM has a dangling dependency ref") unless (dependency_refs - component_refs).empty?

Tempfile.create([input.basename.to_s, ".tmp"], input.dirname.to_s) do |output|
  output.write(JSON.pretty_generate(document))
  output.write("\n")
  output.flush
  output.fsync
  File.rename(output.path, input)
end

puts "CycloneDX SBOM normalized"
