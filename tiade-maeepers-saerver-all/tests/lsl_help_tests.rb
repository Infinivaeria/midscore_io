require 'json'
require 'ripper'

directory = File.expand_path('..', __dir__)
main = File.read(File.join(directory, 'Tiade.Ollama.Ruby.lsl'))
help = File.read(File.join(directory, 'Tiade.Ollama.Ruby.Help.lsl'))

def assert(condition, message)
  raise message unless condition
end

def entries(source, channel)
  pattern = /"(?:\\.|[^"\\])*"|\(string\)CONTROL_CHANNEL|\b(?:c|prefix)\b/
  source.lines.grep(/^\s*notify\(.*\);\s*$/).map do |line|
    expression = line.strip.delete_prefix('notify(').delete_suffix(');')
    assert(expression.gsub(pattern, '').match?(/\A[\s+]*\z/), 'Unexpected help expression')
    expression.scan(pattern).map do |token|
      if token.start_with?('"')
        JSON.parse(token)
      elsif token == '(string)CONTROL_CHANNEL'
        channel.to_s
      else
        "/#{channel} "
      end
    end.join
  end
end

rendered = entries(help, 7)
text = rendered.join("\n")
commands = main.scan(/verb == "([a-z-]+)"/).flatten.uniq
assert(!commands.empty?, 'No commands found in controller')
commands.each { |command| assert(text.include?(command), "Missing command: #{command}") }
helpers = main.scan(/(?:code|method) = "([a-z_]+)(?:\(|")/).flatten.uniq
helpers.each { |helper| assert(text.include?(helper), "Missing helper: #{helper}") }
%w[wait ruby-begin ruby-end].each do |directive|
  assert(text.include?(directive), "Missing directive: #{directive}")
end
puts "PASS: #{commands.length} commands/aliases, storage helpers and card directives documented"

assert(
  help.match?(/if \(page == 0 \|\| page == 9\)\s*\{\s*help_page\(0\);\s*integer topic;\s*for \(topic = 1; topic <= 8; \+\+topic\) help_page\(topic\);/), 'Full-help routing changed'
)
assert(help.include?('else help_page(page);'), 'Numbered help routing missing')
(1..8).each { |topic| assert(text.include?("HELP #{topic}/8"), "Missing topic #{topic}") }
assert(help.include?('num != HELP_LINK_MESSAGE || id != llGetOwner()'), 'Owner/protocol guard missing')
assert(!help.match?(/\bll(?:Say|Shout|Whisper)\s*\(/), 'Help must remain owner-only')
assert(text.include?('END OF HELP.'), 'Completion marker missing')
puts 'PASS: full and numbered help routing, privacy and completion marker'

[7, 2_147_483_647].each do |channel|
  entries(help, channel).each do |entry|
    assert("[tiade] #{entry}\n".bytesize <= 900, "Oversized help entry: #{entry}")
    assert(!entry.match?(%r{/7\b}), 'Hardcoded channel') if channel != 7
  end
end
assert(help.match?(/if \(llStringLength\(gHelpOutput\) \+ llStringLength\(line\) > 900\)\s*flush_help\(\);/),
       'Entry-boundary flush missing')
assert(help.include?('llGetSubString(gHelpOutput, start, start + 899)'), 'Chunk limit changed')
puts 'PASS: chat entry limits at default and maximum positive control channels'

demo = rendered.find { |entry| entry.start_with?('Example Demo contents') }
assert(
  demo.lines.map(&:chomp).drop(1) == ['# comment', 'team secondlife', 'ask What is a prim?', 'wait 2',
                                      'var-set greeting Hello', 'var-get greeting'], 'Invalid command-card example'
)
block = rendered.find { |entry| entry.start_with?('RUBY BLOCK EXAMPLE') }.lines.map(&:chomp).drop(1)
assert(block.first == 'ruby-begin' && block.last == 'ruby-end', 'Invalid Ruby block markers')
assert(Ripper.sexp(block[1...-1].join("\n")), 'Invalid Ruby block syntax')
card = rendered.find { |entry| entry.include?('Ruby-only card example:') }.split("Ruby-only card example:\n").last
assert(Ripper.sexp(card), 'Invalid local-input Ruby card syntax')
puts 'PASS: literal command-card examples and Ruby example syntax'

%w[CONTROL_CHANNEL HELP_LINK_MESSAGE].each do |name|
  pattern = /integer\s+#{name}\s*=\s*(-?\d+)/
  assert(help.match(pattern)[1] == main.match(pattern)[1], "Protocol mismatch: #{name}")
end
puts 'PASS: controller and helper protocol compatibility'
