// Runs against the production script embedded by chatlog_refresh_fixture.py.
window.chatlogTestResults = [];
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const check = (ok, label) => {
  if (!ok) throw Error(label);
  window.chatlogTestResults.push(label);
};
const idle = async () => { await pause(800); interactionDeadline = 0; };
window.runChatlogRefreshTests = async () => {
  await pause(100);
  clearTimeout(refreshTimer);
  const root = document.getElementById('recentMessages');
  const dashboard = document.getElementById('chatlogDashboard');
  const canvas = document.getElementById('heat3dCanvas');
  const range = document.getElementById('chartRange');
  check(root.children.length === 50, 'Initial automatic load');
  const first = root.firstElementChild;
  renderRecentMessages(testMessages);
  check(root.firstElementChild === first, 'Unchanged messages retain DOM nodes');

  root.scrollIntoView({block: 'center'});
  root.scrollTop = 650;
  await idle();
  const anchor = [...root.children].find(item => item.getBoundingClientRect().bottom > root.getBoundingClientRect().top);
  const offset = anchor.getBoundingClientRect().top - root.getBoundingClientRect().top;
  const pageY = window.scrollY;
  testMessages = [makeMessage(101), ...testMessages.slice(0, 49)];
  renderRecentMessages(testMessages);
  check(Math.abs(anchor.getBoundingClientRect().top - root.getBoundingClientRect().top - offset) < 1, 'New messages preserve the visible older message');
  check(Math.abs(window.scrollY - pageY) < 1, 'Recent updates preserve page scroll');
  check(root.firstElementChild.textContent.includes('Message 101'), 'Older-message view still auto-updates');

  const held = root.innerHTML;
  testMessages = Array.from({length: 50}, (_, i) => makeMessage(300 - i));
  renderRecentMessages(testMessages);
  check(root.innerHTML === held && pendingRecentMessages !== null, 'Expired reading anchor keeps its snapshot');
  document.getElementById('showLatestChats').click();
  check(root.scrollTop === 0 && root.firstElementChild.textContent.includes('Message 300'), 'Show latest resumes at newest message');

  root.scrollTop = 300;
  await idle();
  pendingRecentMessages = [makeMessage(301), ...testMessages.slice(0, 49)];
  pointerDown = true;
  const beforeDrag = root.innerHTML;
  applyPendingUpdates();
  check(root.innerHTML === beforeDrag, 'Pointer interaction defers updates');
  pointerDown = false;
  markReadingInteraction();
  await pause(850);
  check(root.firstElementChild.textContent.includes('Message 301'), 'Updates resume automatically after interaction');

  range.focus({preventScroll: true});
  range.value = '7';
  range.dispatchEvent(new Event('change'));
  heat3dRot = {x: 0.2, y: 1.1};
  const row = document.querySelector('#first tr:nth-child(60)');
  window.scrollTo(0, window.scrollY + row.getBoundingClientRect().top - 180);
  await idle();
  const visibleRow = [...document.querySelectorAll('#first tr')].find(node => node.getBoundingClientRect().bottom > document.getElementById('chatAlerts').getBoundingClientRect().bottom);
  const rowLabel = visibleRow.firstElementChild.textContent;
  const rowY = visibleRow.getBoundingClientRect().top;
  const next = new DOMParser().parseFromString(testDashboardHtml, 'text/html');
  next.querySelector('#first table').insertAdjacentHTML('afterbegin', '<tr><td>New row</td><td>' + 'Long value '.repeat(100) + '</td></tr>');
  testDashboardHtml = next.documentElement.outerHTML;
  await refreshDashboard();
  const matchingRow = [...document.querySelectorAll('#first tr')].find(node => node.firstElementChild.textContent === rowLabel);
  check(Math.abs(matchingRow.getBoundingClientRect().top - rowY) < 1, 'Automatic dashboard update anchors the visible table row');
  check(document.getElementById('chatlogDashboard') === dashboard && document.getElementById('recentMessages') === root, 'Dashboard and recent list remain mounted');
  check(document.getElementById('heat3dCanvas') === canvas && document.getElementById('chartRange') === range && range.value === '7' && heat3dRot.y === 1.1, 'Chart controls and rotation survive updates');

  await idle();
  const selection = window.getSelection();
  selection.selectAllChildren(matchingRow.firstElementChild);
  const beforeSelection = dashboard.innerHTML;
  testDashboardHtml = testDashboardHtml.replace('Long value', 'Changed value');
  await refreshDashboard();
  check(dashboard.innerHTML === beforeSelection && pendingDashboard !== null, 'Text selection defers dashboard changes');
  selection.removeAllRanges();
  await pause(850);
  check(pendingDashboard === null, 'Clearing selection resumes automatic updates');

  await idle();
  testDashboardHtml = testDashboardHtml.replace('Changed value', 'Delayed value');
  testDelay = 150;
  const request = refreshDashboard();
  pointerDown = true;
  await request;
  check(pendingDashboard !== null, 'Interaction beginning during a request defers its response');
  pointerDown = false;
  testDelay = 0;
  await idle();
  applyPendingUpdates();

  const beforeConcurrent = testFetchCount;
  testDelay = 100;
  await Promise.all([pollChatlog(), pollChatlog()]);
  clearTimeout(refreshTimer);
  check(testFetchCount === beforeConcurrent + 2, 'Concurrent triggers share one polling cycle');
  testDelay = 0;

  testFailure = true;
  const beforeFailure = document.getElementById('first').innerHTML;
  const messagesBeforeFailure = root.innerHTML;
  await pollChatlog();
  check(document.getElementById('first').innerHTML === beforeFailure && root.innerHTML === messagesBeforeFailure && !refreshInFlight, 'Failed requests preserve content and release the poll');
  testFailure = false;
  const alertHeight = document.getElementById('chatAlerts').getBoundingClientRect().height;
  detectNewChats(101);
  check(document.getElementById('chatAlerts').getBoundingClientRect().height === alertHeight, 'New-chat alerts do not change banner height');
  dismissChatAlerts();
  const requests = testFetchCount;
  await pause(5300);
  check(testFetchCount >= requests + 2, 'Automatic five-second polling recovers after failure');
  clearTimeout(refreshTimer);
  clearTimeout(interactionTimer);
  document.title = 'PASS: chatlog refresh tests';
  return window.chatlogTestResults;
};
document.addEventListener('DOMContentLoaded', () => {
  window.chatlogTestsDone = window.runChatlogRefreshTests().catch(error => {
    document.title = 'FAIL: ' + error.message;
    window.chatlogTestError = error.stack;
    throw error;
  });
});
