#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

root = File.expand_path("..", __dir__)
path = File.join(root, ".github/workflows/release.yml")
workflow = File.read(path, encoding: "UTF-8")

YAML.safe_load(workflow, aliases: true)

required_fragments = [
  "ruby scripts/test_release_workflow.rb",
  "ruby scripts/test_normalize_cyclonedx_sbom.rb",
  "ruby scripts/normalize_cyclonedx_sbom.rb agent_worktree_doctor.cdx.json",
  'git cat-file -t "refs/tags/${GITHUB_REF_NAME}"',
  'expected=(',
  'gh release create "$GITHUB_REF_NAME"',
  'select(.tag_name == env.GITHUB_REF_NAME and .draft == true)',
  'releases/assets/${asset_id}',
  'if [[ "${GITHUB_REF_NAME}" == *-* ]]',
  'release_args+=(--prerelease)',
  'sha256sum --check --strict SHA256SUMS',
  'released-assets',
  'resolve_draft_release_id()',
  'delete_release_with_retry()',
  'retry_delays=(2 4 6 8 10)',
  'matching_count="${#matching_ids[@]}"',
  'multiple matching drafts found for ${GITHUB_REF_NAME}',
  'draft did not become visible after bounded retries',
  'failed to delete draft release ${candidate_id} after bounded retries'
]

required_fragments.each do |fragment|
  abort("release workflow is missing #{fragment.inspect}") unless workflow.include?(fragment)
end

unpinned = workflow.scan(/^\s*uses:\s*([^@\s]+)@([^\s#]+)/).reject do |_name, revision|
  revision.match?(/\A[0-9a-f]{40}\z/)
end
abort("release workflow has unpinned actions") unless unpinned.empty?

abort("stable releases must not always be prereleases") if workflow.match?(/gh release create[^\n]*--prerelease/)
abort("draft releases cannot use the tag endpoint") if workflow.include?("releases/tags/${GITHUB_REF_NAME}")
abort("gh release download cannot resolve drafts") if workflow.include?('gh release download "${GITHUB_REF_NAME}"')
abort("local checksum must not replace the downloaded checksum") if workflow.include?("cp dist/SHA256SUMS released-assets/SHA256SUMS")

resolver = workflow[/resolve_draft_release_id\(\) \{(?<body>.*?)^          \}/m, :body]
abort("release workflow is missing the draft resolver") unless resolver
abort("draft resolver must return exactly one match") unless resolver.include?('[[ "${matching_count}" -eq 1 ]]')
abort("draft resolver must retry zero matches") unless resolver.include?('[[ "${matching_count}" -eq 0 ]]')
abort("draft resolver must fail closed on multiple matches") unless resolver.include?("return 1")

cleanup = workflow[/cleanup\(\) \{(?<body>.*?)^          \}/m, :body]
abort("release workflow is missing cleanup") unless cleanup
abort("cleanup must resolve an exact draft ID") unless cleanup.include?("resolve_draft_release_id")
abort("cleanup must use bounded deletion retries") unless cleanup.include?("delete_release_with_retry")

puts "Release workflow contract passed"
