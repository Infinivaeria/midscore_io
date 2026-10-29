string SERVER = "https://stimky.info";
string DEVICE_TOKEN = "REPLACE_WITH_SL_MESSAGE_DEVICE_TOKEN";
string POLL_ROUTE = "/slmessage/poll";
integer OWNER_CHANNEL = -934833;
string objectId;
string objectSecret;
key objectKey = NULL_KEY;
string localMessage = "";
integer appliedRevision = -1;
integer pollSeconds = 15;
integer failures = 0;
key pending = NULL_KEY;
integer requestStarted;
integer reportDirty = FALSE;
integer lastSuccess;
integer lastStatus;

showStatus()
{
    string lastContact = "never";
    if (lastSuccess > 0) lastContact = (string)(llGetUnixTime() - lastSuccess) + " seconds ago";
    llOwnerSay("SL Message 2.0 | ID: " + objectId + " | prim UUID: " + (string)objectKey
        + " | revision: " + (string)appliedRevision + " | poll: " + (string)pollSeconds
        + " seconds | last success: " + lastContact + " | HTTP: " + (string)lastStatus
        + " | pending: " + (string)(pending != NULL_KEY));
}

pollFailed(integer status, string reason)
{
    lastStatus = status;
    if (failures >= 1000) failures = 990;
    ++failures;
    if (failures == 1 || failures % 10 == 0)
        llOwnerSay("SL Message HTTP " + (string)status + ": " + llGetSubString(reason, 0, 180));
    integer retrySeconds = pollSeconds * failures;
    if (retrySeconds > 120) retrySeconds = 120;
    llSetTimerEvent((float)retrySeconds + llFrand(3.0));
}

integer utf8Bytes(string value)
{
    string encoded = llStringToBase64(value);
    integer size = llStringLength(encoded) * 3 / 4;
    if (llGetSubString(encoded, -1, -1) == "=") --size;
    if (llGetSubString(encoded, -2, -2) == "=") --size;
    return size;
}

integer validDisplay(string body)
{
    if (llJsonValueType(body, ["revision"]) != JSON_NUMBER
        || llJsonValueType(body, ["text"]) != JSON_STRING
        || llJsonValueType(body, ["color"]) != JSON_ARRAY
        || llJsonValueType(body, ["alpha"]) != JSON_NUMBER) return FALSE;
    string revisionValue = llJsonGetValue(body, ["revision"]);
    integer revision = (integer)revisionValue;
    if (revision < 0 || (string)revision != revisionValue) return FALSE;
    if (utf8Bytes(llJsonGetValue(body, ["text"])) > 254) return FALSE;
    if (llGetListLength(llJson2List(llJsonGetValue(body, ["color"]))) != 3) return FALSE;
    integer index;
    for (index = 0; index < 3; ++index)
    {
        if (llJsonValueType(body, ["color", index]) != JSON_NUMBER) return FALSE;
        float component = (float)llJsonGetValue(body, ["color", index]);
        if (component < 0.0 || component > 1.0) return FALSE;
    }
    float alpha = (float)llJsonGetValue(body, ["alpha"]);
    if (alpha < 0.0 || alpha > 1.0) return FALSE;
    return TRUE;
}

integer loadIdentity()
{
    objectKey = llGetKey();
    string storageKey = "slmessage." + (string)objectKey;
    string saved = llLinksetDataRead(storageKey);
    objectId = llJsonGetValue(saved, ["id"]);
    objectSecret = llJsonGetValue(saved, ["secret"]);
    if (llJsonValueType(saved, ["id"]) != JSON_STRING
        || llJsonValueType(saved, ["secret"]) != JSON_STRING
        || llStringLength(objectId) != 36 || llStringLength(objectSecret) != 36
        || (key)objectId == NULL_KEY || (key)objectSecret == NULL_KEY)
    {
        objectId = (string)llGenerateKey();
        objectSecret = (string)llGenerateKey();
        if (llLinksetDataWrite(storageKey, llList2Json(JSON_OBJECT,
            ["id", objectId, "secret", objectSecret])) != 0)
        {
            llOwnerSay("SL Message: could not persist prim identity. Free linkset data space and reset this script.");
            return FALSE;
        }
    }
    return TRUE;
}

sendPoll()
{
    if (pending != NULL_KEY) return;
    if (llGetKey() != objectKey)
    {
        llResetScript();
        return;
    }
    list display = llGetLinkPrimitiveParams(LINK_THIS, [PRIM_TEXT]);
    vector color = llList2Vector(display, 1);
    string payload = llList2Json(JSON_OBJECT, [
        "id", objectId, "secret", objectSecret, "object_key", (string)objectKey,
        "name", llGetObjectName(), "region", llGetRegionName(),
        "applied_revision", appliedRevision, "reported_text", llList2String(display, 0),
        "color", llList2Json(JSON_ARRAY, [color.x, color.y, color.z]),
        "alpha", llList2Float(display, 2), "message", localMessage
    ]);
    pending = llHTTPRequest(SERVER + POLL_ROUTE, [
        HTTP_METHOD, "POST", HTTP_MIMETYPE, "application/json",
        HTTP_CUSTOM_HEADER, "Authorization", "Bearer " + DEVICE_TOKEN,
        HTTP_BODY_MAXLENGTH, 2048
    ], payload);
    if (pending == NULL_KEY)
    {
        pollFailed(0, "Request was throttled or could not start; retrying.");
        return;
    }
    reportDirty = FALSE;
    requestStarted = llGetUnixTime();
    llSetTimerEvent(5.0);
}

default
{
    state_entry()
    {
        SERVER = llStringTrim(SERVER, STRING_TRIM);
        while (llGetSubString(SERVER, -1, -1) == "/")
            SERVER = llDeleteSubString(SERVER, -1, -1);
        if (llSubStringIndex(SERVER, "https://") != 0
            || SERVER == "https://YOUR-SERVER.example"
            || DEVICE_TOKEN == "" || DEVICE_TOKEN == "REPLACE_WITH_SL_MESSAGE_DEVICE_TOKEN")
        {
            llOwnerSay("SL Message: configure SERVER with your HTTPS origin and DEVICE_TOKEN with the server's device token, then reset.");
            return;
        }
        if (!loadIdentity()) return;
        llListen(OWNER_CHANNEL, "", llGetOwner(), "");
        llOwnerSay("SL Message 2.0 prim ID: " + objectId + " | prim UUID: " + (string)objectKey
            + " | owner commands: !status, !poll, !help");
        llSetTimerEvent((float)pollSeconds + llFrand(3.0));
        sendPoll();
    }

    timer()
    {
        if (pending != NULL_KEY && llGetUnixTime() - requestStarted >= 60)
        {
            pending = NULL_KEY;
            pollFailed(0, "Request timed out; retrying.");
            return;
        }
        sendPoll();
    }

    listen(integer channel, string name, key speaker, string message)
    {
        if (speaker != llGetOwner()) return;
        if (llGetSubString(message, 0, 0) == "@")
        {
            if (llGetSubString(message, 1, 36) != objectId
                || llGetSubString(message, 37, 37) != " ") return;
            message = llDeleteSubString(message, 0, 37);
        }
        if (message == "id" || message == "!status")
        {
            showStatus();
            return;
        }
        if (message == "!help")
        {
            llOwnerSay("SL Message 2.0: /" + (string)OWNER_CHANNEL
                + " !status | !poll | !help. Target one prim with @<prim-id> before a command or message."
                + " Pause, scheduling, logs and JSON program state are managed on the server dashboard."
                + " Polling and hover-text updates continue while the server program is paused.");
            return;
        }
        if (message == "!poll")
        {
            reportDirty = TRUE;
            sendPoll();
            return;
        }
        if (utf8Bytes(message) > 512)
        {
            llOwnerSay("SL Message: owner message must fit 512 UTF-8 bytes.");
            return;
        }
        localMessage = message;
        reportDirty = TRUE;
        sendPoll();
    }

    http_response(key request, integer status, list metadata, string body)
    {
        if (request != pending) return;
        pending = NULL_KEY;
        lastStatus = status;
        if (status != 200)
        {
            if (status == 409) appliedRevision = -1;
            pollFailed(status, body);
            return;
        }
        if (!validDisplay(body))
        {
            pollFailed(status, "Invalid server response; update was not applied.");
            return;
        }
        failures = 0;
        lastSuccess = llGetUnixTime();
        if (llJsonValueType(body, ["poll_seconds"]) == JSON_NUMBER)
        {
            pollSeconds = (integer)llJsonGetValue(body, ["poll_seconds"]);
            if (pollSeconds < 15) pollSeconds = 15;
            if (pollSeconds > 120) pollSeconds = 120;
        }
        llSetTimerEvent((float)pollSeconds + llFrand(3.0));
        integer revision = (integer)llJsonGetValue(body, ["revision"]);
        if (revision != appliedRevision)
        {
            vector color = <(float)llJsonGetValue(body, ["color", 0]), (float)llJsonGetValue(body, ["color", 1]), (float)llJsonGetValue(body, ["color", 2])>;
            llSetText(llJsonGetValue(body, ["text"]), color, (float)llJsonGetValue(body, ["alpha"]));
            appliedRevision = revision;
            reportDirty = TRUE;
        }
        if (reportDirty) sendPoll();
    }

    changed(integer change)
    {
        if (change & CHANGED_OWNER) llResetScript();
    }

    on_rez(integer parameter)
    {
        llResetScript();
    }
}