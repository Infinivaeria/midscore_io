"""Print a standalone browser regression page using the actual /chatlog JS and CSS.

Run: python3 tests/chatlog_refresh_fixture.py > /tmp/chatlog-refresh-test.html
Open that file in a browser; the title reports PASS or FAIL. No server or packages
are needed. All fetches use synthetic messages; no production data is accessed.
"""
from pathlib import Path

source = (Path(__file__).resolve().parents[1] / 'src/main.rs').read_text()
start = source.index('<title>Second Life Administrator Chatlog Console</title>')
css = source[source.index('<style>', start):source.index('</style>', start) + 8]
start = source.index('const dashboardData = JSON.parse')
script = source[start:source.index('</script>', start)]
rows = ''.join(f'<tr><td>Row {i}</td><td>Value {i}</td></tr>' for i in range(100))
fixture = f'''<!doctype html><html><head>{css}<title>Chatlog refresh tests</title></head><body>
<aside id="chatAlerts" class="chat-alerts">
<button id="enableChatAlerts">Enable sound &amp; notifications</button>
<button id="dismissChatAlerts" disabled>Dismiss new chats</button>
<button id="showLatestChats" disabled>Show latest chats</button>
<span id="chatAlertMessage" role="status">Watching for new chats.</span>
<div id="chatAlertPermission" class="small">Enable alerts to allow beeps and desktop notifications.</div>
</aside>
<header id="chatlogHeader"><h1>Test chatlog</h1><div class="small">100 messages</div></header>
<main id="chatlogDashboard">
<section id="first"><h2>First table</h2><table>{rows}</table></section>
<section><h2>Second table</h2><table>{rows}</table></section>
<section><div class="section-heading"><h2>Recent messages</h2><span id="recentMessageStatus">Loading</span></div><div id="recentMessages" class="recent-messages"></div></section>
<section><h2>Timeline</h2><select id="chartRange"><option value="all">All</option><option value="7">Week</option></select><div class="chart"><canvas id="timelineChart"></canvas></div></section>
<section><h2>Heatmap</h2><div class="chart"><canvas id="heat3dCanvas"></canvas></div></section>
<section style="height: 900px"><h2>Footer</h2></section>
</main>
<script id="chatlogDashboardData" type="application/json">{{"total_messages":100,"timeline":[],"heatmap":[]}}</script>
'''
setup = r'''
const makeMessage = i => ({ avatar_name: 'Resident', timestamp: i, timestamp_label: String(i), message: `Message ${i} ` + 'text '.repeat(15), tags: ['neutral'] });
let testMessages = Array.from({length: 50}, (_, i) => makeMessage(100 - i));
let testTotal = 100;
let testDelay = 0;
let testFailure = false;
let testFetchCount = 0;
let testDashboardHtml = document.documentElement.outerHTML;
window.fetch = async url => {
  testFetchCount++;
  if (testDelay) await new Promise(resolve => setTimeout(resolve, testDelay));
  if (testFailure) throw Error('Simulated offline');
  return new Response(url.includes('format=recent')
    ? JSON.stringify({ messages: testMessages, total_messages: testTotal, refreshed_at: new Date().toISOString() })
    : testDashboardHtml);
};
'''
tests = (Path(__file__).with_name('chatlog_refresh_tests.js')).read_text()
print(fixture + '<script>' + setup + '</script><script>' + script + '</script><script>' + tests + '</script></body></html>')
