require 'base64'

source = File.read(File.expand_path('../second_life/sl_message.lsl', __dir__))

def assert(condition, message)
  raise message unless condition
end

listen = source.split('listen(integer channel, string name, key speaker, string message)', 2).last
assert(listen, 'Owner listener missing')
listen = listen.split('http_response(', 2).first
assert(
  listen.index('if (speaker != llGetOwner()) return;') < listen.index('if (llGetSubString(message, 0, 0) == "@")'), 'Owner guard must precede targeting'
)
%w[id !status !poll !help].each do |command|
  position = listen.index("message == \"#{command}\"")
  assert(position, "Missing owner command: #{command}")
  assert(listen.index('message = llDeleteSubString(message, 0, 37);') < position, "Targeting must precede #{command}")
end
assert(listen.include?('llGetSubString(message, 1, 36) != objectId'), 'Per-prim target guard missing')
assert(!source.match?(/\bll(?:Say|Shout|Whisper)\s*\(/), 'Diagnostics must remain owner-only')
assert(source.include?('HTTP_CUSTOM_HEADER, "Authorization", "Bearer " + DEVICE_TOKEN'),
       'Device bearer authentication missing')
assert(!source.include?('slmessage-admin') && !source.include?('/runtime'),
       'LSL must not contain admin credentials or runtime API access')
assert(source.include?('string POLL_ROUTE = "/slmessage/poll";'), 'Polling route changed')
assert(source.include?('"slmessage." + (string)objectKey'), 'Persisted per-prim identity namespace changed')
puts 'PASS: owner-only commands, targeting, credential boundaries and per-prim identity'

send_poll = source.split('sendPoll()', 2).last.split('default', 2).first
assert(send_poll.include?('if (pending != NULL_KEY) return;'), 'Overlapping-request guard missing')
assert(send_poll.index('if (pending == NULL_KEY)') < send_poll.index('reportDirty = FALSE;'),
       'Throttled requests must not clear pending reports')
assert(listen.include?("localMessage = message;\n        reportDirty = TRUE;\n        sendPoll();"),
       'Owner messages must queue a follow-up report')
response = source.split('http_response(', 2).last
assert(response.include?('if (request != pending) return;'), 'Stale-response guard missing')
assert(response.include?('if (reportDirty) sendPoll();'), 'Queued reports and acknowledgments must be flushed')
assert(response.index('if (!validDisplay(body))') < response.index('llSetText('),
       'Display validation must precede applying updates')
assert(source.include?('pollFailed(0, "Request timed out; retrying.");'), 'Timeout retry missing')
assert(source.include?('pollFailed(0, "Request was throttled or could not start; retrying.");'),
       'Local throttle retry missing')
assert(send_poll.include?("requestStarted = llGetUnixTime();\n    llSetTimerEvent(5.0);"),
       'Pending requests need an independent watchdog timer')
assert(source.include?('pending != NULL_KEY && llGetUnixTime() - requestStarted >= 60'),
       'Pending requests must expire at the 60-second deadline')
assert(source.include?("if (failures >= 1000) failures = 990;\n    ++failures;"),
       'Failure counter must retain one warning per ten failures after saturation')
assert(
  source.include?('if (pollSeconds < 15) pollSeconds = 15;') && source.include?('if (pollSeconds > 120) pollSeconds = 120;'), 'Polling interval bounds changed'
)
puts 'PASS: request serialization, queued messages, acknowledgment, retry and polling guards'

failures = 0
warnings = []
2_050.times do |index|
  failures = 990 if failures >= 1000
  failures += 1
  warnings << index + 1 if failures == 1 || (failures % 10).zero?
end
assert(warnings == [1] + (10..2050).step(10).to_a, 'Long outages must not flood owner chat')
puts 'PASS: warning cadence remains bounded through 2,050 failures'

assert(source.include?('integer size = llStringLength(encoded) * 3 / 4;'), 'UTF-8 length formula changed')
assert(source.include?('if (llGetSubString(encoded, -1, -1) == "=") --size;'),
       'First Base64 padding correction missing')
assert(source.include?('if (llGetSubString(encoded, -2, -2) == "=") --size;'),
       'Second Base64 padding correction missing')
assert(source.include?('utf8Bytes(message) > 512'), 'Owner message byte limit differs from server')
assert(source.include?('utf8Bytes(llJsonGetValue(body, ["text"])) > 254'), 'Hover-text byte limit differs from server')
[0, 1, 2, 3, 253, 254, 255, 510, 511, 512, 513].each do |length|
  ['a' * length, "\u00e9" * length, "\u{1f315}" * length].each do |value|
    encoded = Base64.strict_encode64(value)
    calculated = encoded.length * 3 / 4 - (encoded.end_with?('=') ? 1 : 0) - (encoded.end_with?('==') ? 1 : 0)
    assert(calculated == value.bytesize, 'UTF-8 Base64 boundary mismatch')
  end
end
puts 'PASS: UTF-8 byte formula at message and hover-text boundaries'
