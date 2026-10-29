const {readFileSync} = require('node:fs');
const {runInNewContext} = require('node:vm');
const assert = require('node:assert/strict');
const {test} = require('node:test');
const rust = readFileSync(require('node:path').join(__dirname,'../src/main.rs'),'utf8');
const page = rust.split('app.at("/palmorganizer")')[1].split('r###"')[1].split('"###')[0];
const source = page.match(/<script>\s*([\s\S]*?)<\/script>/)[1];
const context = {module:{exports:{}},Intl,Date};
runInNewContext(source,context);
const {pacificDay,nextAlarm,elapsed,remaining,fresh,validate,setSwitch,occursOn,automationWorkerSource,applyAutomation,sessionWriter} = context.module.exports;

test('calendar recurrence follows Pacific calendar dates across DST with inclusive end dates',() => {
  const event = {date:'2026-03-01',repeat:'weekly',until:'2026-03-15'};
  assert.equal(occursOn(event,'2026-02-22'),false);
  assert.equal(occursOn(event,'2026-03-08'),true);
  assert.equal(occursOn(event,'2026-03-09'),false);
  assert.equal(occursOn(event,'2026-03-15'),true);
  assert.equal(occursOn(event,'2026-03-22'),false);
  assert.equal(occursOn({date:'2026-01-01',repeat:'daily'},'2027-01-01'),true);
  assert.equal(occursOn({date:'2026-01-01'},'2026-01-02'),false);
});

test('programs can schedule and checkpoint memory atomically across runs',() => {
  const original = fresh();
  const updated = applyAutomation(original,[{kind:'event',event:{title:'Review',date:'2026-10-29',time:'09:00',endTime:'10:00',repeat:'weekly'}},{kind:'memory',key:'step',value:{counter:'12345678901234567890'}}],'2026-10-29',()=>'event',0);
  assert.equal(updated.events[0].repeat,'weekly');
  assert.equal(updated.automationMemory.step.counter,'12345678901234567890');
  assert.equal(original.automationMemory,undefined);
  assert.equal(validate(JSON.parse(JSON.stringify(updated))).events.length,1);
  for (const event of [{title:'Bad',time:'10:00',endTime:'09:00'},{title:'Bad',repeat:'monthly'},{title:'Bad',until:'2020-01-01'}]) {
    assert.throws(() => applyAutomation(original,[{kind:'event',event}],'2026-10-29',()=>'event',0));
  }
  assert.throws(() => applyAutomation(original,[{kind:'memory',key:'__proto__',value:{}}],'2026-10-29',()=>'event',0));
});

test('switch counts track off-to-on transitions only and old backups remain valid',() => {
  const data = fresh(), item = {id:'water',title:'Water',on:false,color:'#176b53'};
  data.switches.push(item); validate(data);
  setSwitch(item,true); setSwitch(item,true); setSwitch(item,false);
  assert.equal(item.toggleCount,1);
  assert.equal(item.on,false);
  setSwitch(item,true); setSwitch(item,false);
  assert.equal(item.toggleCount,2);
  assert.equal(validate(JSON.parse(JSON.stringify(data))).switches[0].toggleCount,2);
  item.toggleCount = -1; assert.throws(() => validate(data));
  item.toggleCount = Number.MAX_SAFE_INTEGER; assert.throws(() => setSwitch(item,true));
  assert.equal(item.on,false);
  item.on = true; setSwitch(item,false);
  assert.equal(item.toggleCount,Number.MAX_SAFE_INTEGER);
  assert.equal(item.on,false);
});

test('session autosaves serialize revisions and preserve edits made during a save',async () => {
  const writes = [], pending = [];
  const writer = sessionWriter(command => { writes.push(command); return new Promise(resolve => pending.push(resolve)); },() => {});
  let data = fresh(); writer.open({id:'work',revision:3}); writer.dirty();
  const first = writer.flush(() => data);
  data.notes['2026-10-23'] = 'Changed during save'; writer.dirty();
  assert.equal(writes[0].data.notes['2026-10-23'],undefined);
  pending.shift()({revision:4}); await first;
  assert.equal(writer.unsaved(),true);
  const second = writer.flush(() => data);
  assert.equal(writes[1].revision,4);
  assert.equal(writes[1].data.notes['2026-10-23'],'Changed during save');
  pending.shift()({revision:5}); await second;
  assert.equal(writer.unsaved(),false);
  writer.open({id:'home',revision:0}); writer.dirty();
  const third = writer.flush(() => fresh());
  assert.equal(writes[2].id,'home'); assert.equal(writes[2].revision,0);
  pending.shift()({revision:1}); await third;
});

test('session autosave retains unsaved edits after failures and blocks conflict retries',async () => {
  let calls = 0;
  const writer = sessionWriter(async () => { calls++; const error = new Error('Conflict'); error.status = 409; throw error; },() => {});
  writer.open({id:'work',revision:0}); writer.dirty();
  await assert.rejects(writer.flush(() => fresh()));
  assert.equal(writer.unsaved(),true);
  await assert.rejects(writer.flush(() => fresh()));
  assert.equal(calls,1);
});

test('Pacific calendar day differs from UTC and follows DST',() => {
  assert.equal(pacificDay(Date.parse('2026-01-01T07:59:00Z')),'2025-12-31');
  assert.equal(pacificDay(Date.parse('2026-07-01T07:00:00Z')),'2026-07-01');
});
test('alarms follow winter PST and summer PDT',() => {
  assert.equal(nextAlarm('08:00',Date.parse('2026-01-01T12:00:00Z')),Date.parse('2026-01-01T16:00:00Z'));
  assert.equal(nextAlarm('08:00',Date.parse('2026-07-01T12:00:00Z')),Date.parse('2026-07-01T15:00:00Z'));
});
test('nonexistent spring time schedules the next valid day; repeated fall time stays future',() => {
  assert.equal(nextAlarm('02:30',Date.parse('2026-03-08T09:59:00Z')),Date.parse('2026-03-09T09:30:00Z'));
  assert.equal(nextAlarm('01:30',Date.parse('2026-11-01T08:40:00Z')),Date.parse('2026-11-01T09:30:00Z'));
  assert.equal(nextAlarm('01:30',Date.parse('2026-11-01T08:30:00Z'),'2026-11-01'),Date.parse('2026-11-02T09:30:00Z'));
  assert.throws(() => nextAlarm('24:00',Date.now()));
});
test('stopwatch and countdown survive reload and late browser callbacks',() => {
  assert.equal(elapsed({elapsed:3000,since:1000},9000),11000);
  assert.equal(elapsed({elapsed:3000,since:null},9000),3000);
  assert.equal(remaining({remaining:5000,end:6000},9000),0);
  assert.equal(remaining({remaining:5000,end:null},9000),5000);
});
test('backup validation rejects malformed data and impossible dates',() => {
  assert.equal(validate(fresh()).version,1);
  const data = fresh(); data.events.push({id:'event',title:'Meeting',date:'2026-02-30',time:'08:00'});
  assert.throws(() => validate(data));
  const timer = fresh(); timer.timer.end = 'tomorrow'; assert.throws(() => validate(timer));
  const switches = fresh(); switches.switches.push({id:'switch',title:'Water',on:true,color:'url(bad)'}); assert.throws(() => validate(switches));
});

test('automation executes JavaScript branching, loops, arbitrary precision integers and organizer commands',async () => {
  const messages = [], sandbox = {postMessage:message => messages.push(message)};
  runInNewContext(automationWorkerSource,{self:sandbox});
  await sandbox.onmessage({data:{snapshot:{today:'2026-10-23'},code:'let value = 1n; for (let count = 1n; count <= 30n; count++) value *= count; if (value > 0n) print(value.toString()); organizer.addTask("Computed");'}});
  assert.equal(messages[0].ok,true);
  assert.equal(messages[0].output[0],'265252859812191058636308480000000');
  assert.equal(messages[0].commands[0].title,'Computed');
});
test('automation stages valid actions atomically without mutating original data',() => {
  const original = fresh(); original.switches.push({id:'water',title:'Water',on:false,color:'#176b53'});
  const result = applyAutomation(original,[{kind:'switch',id:'water',on:true},{kind:'task',title:'Stretch'},{kind:'note',text:'Done'},{kind:'timer',seconds:5}],'2026-10-23',()=>'new-task',1000);
  assert.equal(original.tasks.length,0); assert.equal(original.switches[0].on,false);
  assert.equal(result.switches[0].on,true); assert.equal(result.tasks[0].date,'2026-10-23'); assert.equal(result.timer.end,6000); assert.equal(result.notes['2026-10-23'],'Done');
  assert.throws(()=>applyAutomation(original,[{kind:'task',title:'Valid'},{kind:'timer',seconds:-1}],'2026-10-23',()=>'task',1000));
  assert.equal(original.tasks.length,0);
});
test('automation catches errors and limits output and actions',async () => {
  for (const code of ['throw new Error("oops")','for(let count=0;count<101;count++) print(count)','for(let count=0;count<101;count++) organizer.addTask("Task")']) {
    const messages = [], sandbox = {postMessage:message=>messages.push(message)};
    runInNewContext(automationWorkerSource,{self:sandbox}); await sandbox.onmessage({data:{snapshot:{},code}});
    assert.equal(messages[0].ok,false); assert.equal(messages[0].commands,undefined);
  }
  const invalid = fresh(); invalid.automationCode = 42; assert.throws(()=>validate(invalid));
  assert.equal(validate(fresh()).version,1);
});