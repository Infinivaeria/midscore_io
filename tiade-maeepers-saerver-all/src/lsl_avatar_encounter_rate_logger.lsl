////!!!!!!!!!!!!!!!!!!MAIN.rb - top - v7.0-simplified)
/// for MIDSCORE.IO

// Simplified: scan on a fixed timer, send the results right away. No retry/backoff,
// no in-flight tracking - each timer tick does exactly one scan and one POST.

string VERSION = "HUDlink.main-v7.0-simplified";
string avatar_frequency_url = "https://stimky.info/avatarfrequency";
float SCAN_INTERVAL = 30.0;
float SCAN_RADIUS = 96.0;

list avatar_frequency = []; // [key, name, count] triples, reset every scan
integer scan_count = 0;

integer avatar_index(key avatar_id)
{
    integer i;
    for (i = 0; i < llGetListLength(avatar_frequency); i += 3)
    {
        if (llList2Key(avatar_frequency, i) == avatar_id)
            return i;
    }
    return -1;
}

record_avatar_encounter(key avatar_id, string avatar_name)
{
    integer index = avatar_index(avatar_id);
    if (index == -1)
    {
        avatar_frequency += [avatar_id, avatar_name, 1];
    }
    else
    {
        avatar_frequency = llListReplaceList(avatar_frequency,
            [avatar_name, llList2Integer(avatar_frequency, index + 2) + 1], index + 1, index + 2);
    }
}

string avatar_frequency_payload()
{
    list records = [];
    integer i;
    for (i = 0; i < llGetListLength(avatar_frequency); i += 3)
    {
        records += llList2Json(JSON_OBJECT, [
            "uuid", (string)llList2Key(avatar_frequency, i),
            "name", llList2String(avatar_frequency, i + 1),
            "count", llList2Integer(avatar_frequency, i + 2)
        ]);
    }
    string avatars_json = "[" + llDumpList2String(records, ",") + "]";
    return llList2Json(JSON_OBJECT, [
        "source", "lsl_avatar_sensor",
        "captured_by", llGetUsername(llGetOwner()),
        "sim_name", llGetRegionName(),
        "scan_at", llGetUnixTime(),
        "scan_count", scan_count,
        "scan_interval_seconds", SCAN_INTERVAL,
        "avatars_seen", llGetListLength(avatar_frequency) / 3,
        "avatars", avatars_json
    ]);
}

send_avatar_frequency()
{
    llHTTPRequest(avatar_frequency_url,
        [HTTP_METHOD, "POST", HTTP_MIMETYPE, "application/json"],
        avatar_frequency_payload());
}

default
{
    state_entry()
    {
        llOwnerSay(":: " + VERSION + " active, scanning every " + (string)SCAN_INTERVAL + "s ::");
        llSetTimerEvent(SCAN_INTERVAL);
    }

    on_rez(integer start_parameter)
    {
        llResetScript();
    }

    timer()
    {
        llSensor("", NULL_KEY, AGENT, SCAN_RADIUS, PI);
    }

    sensor(integer num)
    {
        avatar_frequency = [];
        scan_count += 1;
        integer i;
        for (i = 0; i < num; i++)
        {
            record_avatar_encounter(llDetectedKey(i), llDetectedName(i));
        }
        send_avatar_frequency();
    }

    no_sensor()
    {
        avatar_frequency = [];
        scan_count += 1;
        send_avatar_frequency();
    }
}