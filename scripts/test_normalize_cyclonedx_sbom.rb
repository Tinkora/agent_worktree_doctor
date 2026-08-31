#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "open3"
require "tempfile"

root_dir = File.expand_path("..", __dir__)
normalizer = File.join(root_dir, "scripts", "normalize_cyclonedx_sbom.rb")
root_ref = "path+file:///home/runner/work/agent_worktree_doctor#0.1.0-alpha.2"
target_ref = "#{root_ref} target-0"
local_component_ref = "path+file:///home/runner/work/local_helper#1.0.0"
registry_ref = "registry+https://github.com/rust-lang/crates.io-index#anyhow@1.0.104"
fixture = {
  "bomFormat" => "CycloneDX",
  "specVersion" => "1.5",
  "metadata" => { "component" => {
    "type" => "application", "bom-ref" => root_ref,
    "name" => "agent_worktree_doctor", "version" => "0.1.0-alpha.2",
    "purl" => "pkg:cargo/agent_worktree_doctor@0.1.0-alpha.2?download_url=file://.",
    "components" => [{
      "type" => "library", "bom-ref" => target_ref,
      "name" => "agent_worktree_doctor", "version" => "0.1.0-alpha.2",
      "purl" => "pkg:cargo/agent_worktree_doctor@0.1.0-alpha.2?download_url=file://.#src/lib.rs"
    }]
  } },
  "components" => [
    { "type" => "library", "bom-ref" => local_component_ref, "name" => "local_helper", "version" => "1.0.0", "purl" => "pkg:cargo/local_helper@1.0.0?download_url=file:///home/runner/work/local_helper#vendor/local" },
    { "type" => "library", "bom-ref" => registry_ref, "name" => "anyhow", "version" => "1.0.104", "purl" => "pkg:cargo/anyhow@1.0.104" }
  ],
  "dependencies" => [
    { "ref" => root_ref, "dependsOn" => [local_component_ref, registry_ref] },
    { "ref" => local_component_ref, "dependsOn" => [] },
    { "ref" => registry_ref, "dependsOn" => [] }
  ]
}

def normalizer_accepts?(normalizer, document)
  Tempfile.create(["invalid-sbom", ".cdx.json"]) do |file|
    file.write(JSON.pretty_generate(document))
    file.close
    _stdout, _stderr, status = Open3.capture3("ruby", normalizer, file.path)
    return status.success?
  end
end

Tempfile.create(["sbom", ".cdx.json"]) do |file|
  file.write(JSON.pretty_generate(fixture))
  file.close
  stdout, stderr, status = Open3.capture3("ruby", normalizer, file.path)
  abort("normalizer failed: #{stdout}#{stderr}") unless status.success?

  text = File.read(file.path, encoding: "UTF-8")
  abort("normalized SBOM contains a local URI") if text.match?(%r{(?:path\+)?file://})
  abort("normalized SBOM contains a runner path") if text.include?("/home/runner/")
  document = JSON.parse(text)
  components = [document.dig("metadata", "component"), *document.fetch("components")]
  components += document.dig("metadata", "component", "components")
  refs = components.map { |component| component.fetch("bom-ref") }
  abort("normalized SBOM has duplicate refs") unless refs.uniq.length == refs.length
  dependency_refs = document.fetch("dependencies").flat_map { |dependency| [dependency.fetch("ref"), *dependency.fetch("dependsOn")] }
  abort("normalized SBOM has dangling refs") unless (dependency_refs - refs).empty?
end

duplicate = Marshal.load(Marshal.dump(fixture))
duplicate.fetch("components").last["bom-ref"] = "pkg:cargo/agent_worktree_doctor@0.1.0-alpha.2"
abort("normalizer accepted duplicate component refs") if normalizer_accepts?(normalizer, duplicate)

dangling = Marshal.load(Marshal.dump(fixture))
dangling.fetch("dependencies").first.fetch("dependsOn") << "pkg:cargo/missing@1.0.0"
abort("normalizer accepted a dangling dependency ref") if normalizer_accepts?(normalizer, dangling)

puts "CycloneDX SBOM normalization contract passed"
