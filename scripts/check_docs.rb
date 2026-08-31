# frozen_string_literal: true

require "pathname"
require "uri"

root = Pathname(__dir__).join("..").expand_path
pairs = [
  %w[README.md README.zh-CN.md],
  %w[CONTRIBUTING.md CONTRIBUTING.zh-CN.md],
  %w[SECURITY.md SECURITY.zh-CN.md],
  %w[SUPPORT.md SUPPORT.zh-CN.md],
  %w[CODE_OF_CONDUCT.md CODE_OF_CONDUCT.zh-CN.md],
  %w[docs/PRODUCT_SPEC.md docs/PRODUCT_SPEC.zh-CN.md]
]
errors = []
pairs.flatten.each { |path| errors << "missing bilingual document: #{path}" unless root.join(path).file? }
root.glob("**/*.md").sort.each do |document|
  document.read(encoding: "UTF-8").scan(/\[[^\]]*\]\(([^)]+)\)/).flatten.each do |raw|
    target = raw.strip.split("#", 2).first
    next if target.empty? || target.start_with?("mailto:") || URI.parse(target).scheme
    errors << "broken internal link: #{target}" unless document.dirname.join(URI::DEFAULT_PARSER.unescape(target)).cleanpath.exist?
  rescue URI::InvalidURIError
    errors << "invalid link: #{raw}"
  end
end
abort(errors.join("\n")) unless errors.empty?
puts "Documentation contracts passed"
