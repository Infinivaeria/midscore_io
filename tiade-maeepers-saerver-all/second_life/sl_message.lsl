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
    requestStarted = llGetUnixTime();
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
        llOwnerSay("SL Message prim ID: " + objectId + " | prim UUID: " + (string)objectKey);
        llSetTimerEvent((float)pollSeconds + llFrand(3.0));
        sendPoll();
    }

    timer()
    {
        if (pending != NULL_KEY && llGetUnixTime() - requestStarted > 60) pending = NULL_KEY;
        sendPoll();
    }

    listen(integer channel, string name, key speaker, string message)
    {
        if (speaker != llGetOwner()) return;
        if (message == "id")
        {
            llOwnerSay("SL Message prim ID: " + objectId + " | prim UUID: " + (string)objectKey);
            return;
        }
        if (llGetSubString(message, 0, 0) == "@")
        {
            if (llGetSubString(message, 1, 36) != objectId
                || llGetSubString(message, 37, 37) != " ") return;
            message = llDeleteSubString(message, 0, 37);
        }
        if (llStringLength(llStringToBase64(message)) > 680)
        {
            llOwnerSay("SL Message: owner message must fit 510 UTF-8 bytes.");
            return;
        }
        localMessage = message;
        sendPoll();
    }

    http_response(key request, integer status, list metadata, string body)
    {
        if (request != pending) return;
        pending = NULL_KEY;
        if (status != 200)
        {
            if (status == 409) appliedRevision = -1;
            ++failures;
            if (failures == 1 || failures % 10 == 0) llOwnerSay("SL Message HTTP " + (string)status + ": " + llGetSubString(body, 0, 180));
            integer retrySeconds = pollSeconds * failures;
            if (retrySeconds > 120) retrySeconds = 120;
            llSetTimerEvent((float)retrySeconds + llFrand(3.0));
            return;
        }
        string revisionValue = llJsonGetValue(body, ["revision"]);
        string text = llJsonGetValue(body, ["text"]);
        if (llJsonValueType(body, ["revision"]) != JSON_NUMBER || llJsonValueType(body, ["text"]) != JSON_STRING
            || llJsonValueType(body, ["color"]) != JSON_ARRAY || llJsonValueType(body, ["alpha"]) != JSON_NUMBER)
        {
            llOwnerSay("SL Message: invalid server response; update was not applied.");
            return;
        }
        failures = 0;
        if (llJsonValueType(body, ["poll_seconds"]) == JSON_NUMBER)
        {
            pollSeconds = (integer)llJsonGetValue(body, ["poll_seconds"]);
            if (pollSeconds < 15) pollSeconds = 15;
            if (pollSeconds > 120) pollSeconds = 120;
        }
        llSetTimerEvent((float)pollSeconds + llFrand(3.0));
        integer revision = (integer)revisionValue;
        if (revision != appliedRevision)
        {
            vector color = <(float)llJsonGetValue(body, ["color", 0]), (float)llJsonGetValue(body, ["color", 1]), (float)llJsonGetValue(body, ["color", 2])>;
            llSetText(text, color, (float)llJsonGetValue(body, ["alpha"]));
            appliedRevision = revision;
            sendPoll();
        }
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