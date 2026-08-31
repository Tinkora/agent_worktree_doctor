# frozen_string_literal: true

require "yaml"

root = File.expand_path("..", __dir__)
workflow = File.read(File.join(root, ".github/workflows/release.yml"), encoding: "UTF-8")
YAML.safe_load(workflow, aliases: true)
[
  'git cat-file -t "refs/tags/${GITHUB_REF_NAME}"',
  "ruby scripts/test_release_workflow.rb",
  "expected=(",
  "sha256sum --check --strict SHA256SUMS",
  "cmp dist/SHA256SUMS downloaded/SHA256SUMS",
  'gh release download "${GITHUB_REF_NAME}"',
  'gh release delete "${GITHUB_REF_NAME}" --repo "${GITHUB_REPOSITORY}" --yes',
  'args+=(--prerelease)'
].each { |fragment| abort("missing release contract: #{fragment}") unless workflow.include?(fragment) }
unpinned = workflow.scan(/^\s*uses:\s*[^@\s]+@([^\s#]+)/).flatten.reject { |revision| revision.match?(/\A[0-9a-f]{40}\z/) }
abort("release actions must use commit SHAs") unless unpinned.empty?
puts "Release workflow contract passed"
