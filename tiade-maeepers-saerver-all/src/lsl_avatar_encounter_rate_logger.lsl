////!!!!!!!!!!!!!!!!!!MAIN.rb - top - v2.0a - 2022-09-19)
/// for MIDSCORE.IO



// AS IT STANDS, IF YOU HAVE MORE THAN ONE AVATAR USING THIS IN THE SAME PROXIMITY THERE WILL BE DUPLICATE MESSAGES SEND TO https://hudl.ink

//Remember to backup accordingly. Because of the second life server's function,
// we DON'T know when or how we could lose data--BUT IT HAS BEEN DONE BEFORE, or
// data corruption in some way while in server memory or execution or the serves
// save data to disk. Can happen during restarts and second life's sims in some parcels etc fail.
// Data is lost somehow there. We do not know the underlying details and focus on programming at a very "high level. Remember your computer science teachings in Academia...
//------------------------------------------ END THE MOST IMPORTANT PARAGRAPHS TO UNDERSTAND ^

//# HUDlink Interface Overlay - Prototype of the main loop. - By ArityWolf - Last Changed 2022-09-19 (year - month in two digits; and day)

//Version v4.x - implement is_object hash json value
// Which tells you if an object/avatar is indeed an object, since the scanner doesn't take that into much consideration from what it seems...

// Version v4.1a - implemented link parsers, character escaping; everything seems to parse correctly now

// HUD(main) - v1.0f : A HUDlink second life logger (primarily, the chat channel (llSay(...))) - note: versions can go higher from here.
// Version 1.0f - Implementation of the PartitionedArray and ManagedPartitonedArray classes in Ruby, in which this is the first successful attempt at field testing
// NOTE: Biggest problem I'm having so far is managing the code updates since we don't yet fully know how to execute scripts fully remotely in Second Life

//BIG NOTE:: need to make avatar usage easy to change code and manage, etc. If any avatar with any other, only one hudlink object logs text... etc...


// END untouched code that you copy and pasted: (ArityWolf):
   string ASCII = "             \n                   !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";

//------- START main() code// -- certain variables may be deprecated or unused! 2022-09-19
//string VERSION_NUM = "v4.2-alpha";
string VERSION = "HUDlink.main.patch_version-v6.0.1";
string SMALL_VERSION = "!!!!!!!!!!!HUDlink-main_loop-v6.0.1-majorpatch.lsl";

string node = "@ArityWolf(@the_field)"; // by default, say this is ArityWolf. We have different nodes to notify the owner, where the chat came from as just a kind of "label".
integer partition_size = 0;
integer chat_handle;
integer channel = 0;
integer SIZE = 250;
integer R = 31;
list data_items = [];
integer DEBUG = FALSE;
string NULL_STRING = "";
integer MAX_STACK_SIZE = 1;
string captured_by = NULL_STRING;
integer detected = TRUE;
string avatar_message = "";
key temp_avatar_uuid;
list avatar_frequency = [];
integer avatar_scan_count = 0;
integer avatar_total_encounters = 0;
integer avatar_last_scan_at = 0;
float avatar_scan_interval = 30.0;
string avatar_frequency_url = "https://stimky.info/avatarfrequency";
string ESCAPE_CHARACTER = "\\";
string ESCAPE_CHARACTER_REPLACE = "\\\\";
integer URL_PARSING = FALSE;
integer URL_PARSING_PUBLIC = FALSE;
string parsed_message;
integer OUTPUT_BOOTUP = FALSE;
float benchmark_startup_time_start;
float benchmark_startup_time_end;

float bytes_to_kilobytes(integer bytes) { return bytes / 1024.0; }
float time() { return (float)llGetUnixTime() * 1000.0; }
float calculate_delta(float end, float start) { return end - start; }
string escape(string value) { return value; }
string unescape(string value) { return value; }

integer code_channel = 11;
/// START
// List all integer channels here to be mohnitored (currently wip because of the comments as you'll see)
/// ...
/// ...
/// ...
/// ... ...
/// END

integer ADMIN_SAY = FALSE;
admin_say(string say)
{
    if (ADMIN_SAY)
       { llOwnerSay(say);}
}
    

//string single_double_quote = "\\\\"; // probably not working right

   integer ord(string chr)
   {
       if(llStringLength(chr) != 1) return -1;
       if(chr == " ") return 32;
       return llSubStringIndex(ASCII, chr);
   }


// Associated array == hash
integer hash_code(string skey)
{
    integer hash=0;    
    integer slen = llStringLength(skey);
    
    integer i;
    for (i=0; i < slen; i++) 
    {
        hash=(R * hash + (integer)ord(llGetSubString(skey, i, i))) % SIZE;
        
    } 
    return hash;   
}

string str_replace(string str, string search, string replace) {
    return llDumpList2String(llParseStringKeepNulls(str, [search], []), replace);
}


string search(string skey, list table)
{
    integer hash_index = hash_code(skey);
    
    if (hash_index == 0)
    {
     if (llJsonGetValue(llList2String(table, hash_index), ["key"]) == skey)
     {
        return llJsonGetValue(llList2String(table, hash_index), ["data"]);   
     }   
    }    
    

    while(hash_index != 0) 
    {

        if (llJsonGetValue(llList2String(table, hash_index), ["key"]) == skey)
        {
            return llJsonGetValue(llList2String(table, hash_index), ["data"]);
        }
        hash_index += 1;
        hash_index %= SIZE;
       
      
    }
    
    if (hash_index == 0)
    {
        if (llJsonGetValue(llList2String(table, hash_index), ["key"]) == skey) 
        {
          return llJsonGetValue(llList2String(table, hash_index), ["data"]);   
         } 
    }
    return NULL_STRING;
} 






list remove_null_strings(list table)
{
    integer length=llGetListLength(table);
    integer incrementor=0;
    list initial_table = table;
    list final_table;
    integer new_counter=0;
    while(incrementor < length)
    {
        if (llList2String(initial_table, incrementor) != "")
            {
            if (DEBUG==TRUE)
                debug((string)llList2String(initial_table,incrementor));
            else  
                debug ((string)llList2String(initial_table, incrementor)); 
                
             final_table = final_table + llList2String(initial_table, incrementor);  
            }
            incrementor++; 
    }
    
    return final_table;
}

push_to_server(list table)
{
   list post_table = remove_null_strings(table);
   
   
   key http_req=llHTTPRequest("https://stimky.info/sl_logger", [HTTP_METHOD, "POST"], llDumpList2String(post_table, "\n"));    
    data_items = [];
   admin_say("Hudl.ink: Table uploaded... from: " + node);
    }

list insert3(string timestamp, string avatar_name, string avatar_id, string message, float x_pos, float y_pos, float z_pos, string sim_name, list table)
    {
        if (search(timestamp, table) == NULL_STRING)
        {
            string item = "[{'timestamp':, 'avatar_name':, 'avatar_id':, 'message':', 'xpos':', 'ypos':', 'zpos':', 'sim_name':', 'avatar_id':'}]";
            item = llJsonSetValue(item, ["timestamp"], timestamp);
            item = llJsonSetValue(item, ["avatar_name"], avatar_name);
            item = llJsonSetValue(item, ["avatar_id"], avatar_id);
            item = llJsonSetValue(item, ["message"], message);
            
            item = llJsonSetValue(item, ["x_pos"], (string)x_pos);
            item = llJsonSetValue(item, ["y_pos"], (string)y_pos);
            item = llJsonSetValue(item, ["z_pos"], (string)z_pos);
            item = llJsonSetValue(item, ["sim_name"], (string)sim_name);
           
            integer hash_index = hash_code(timestamp);
           while( llList2Integer(table, hash_index) != 0 && llJsonGetValue( llList2String(table, hash_index), [timestamp] ) != NULL_STRING ) 
    {
       hash_index += 1;
       hash_index %= SIZE;           
    }
      return llListInsertList(table, [item], hash_index);
       
    }
     return table;
     
    
}

list insert4(string timestamp, string avatar_name, string avatar_id, string message, float x_pos, float y_pos, float z_pos, string sim_name, string captured_by, list table)
    {
        if (search(timestamp, table) == NULL_STRING)
        {
            string item = "[{'timestamp':, 'avatar_name':, 'avatar_id':, 'message':', 'xpos':', 'ypos':', 'zpos':', 'sim_name':', 'avatar_id':', 'captured_by':'}]";
            item = llJsonSetValue(item, ["timestamp"], timestamp);
            item = llJsonSetValue(item, ["avatar_name"], avatar_name);
            item = llJsonSetValue(item, ["avatar_id"], avatar_id);
            item = llJsonSetValue(item, ["message"], message);
            
            item = llJsonSetValue(item, ["x_pos"], (string)x_pos);
            item = llJsonSetValue(item, ["y_pos"], (string)y_pos);
            item = llJsonSetValue(item, ["z_pos"], (string)z_pos);
            item = llJsonSetValue(item, ["sim_name"], sim_name);
            item = llJsonSetValue(item, ["captured_by"], captured_by);
           
            integer hash_index = hash_code(timestamp);
           while( llList2Integer(table, hash_index) != 0 && llJsonGetValue( llList2String(table, hash_index), [timestamp] ) != NULL_STRING ) 
    {
       hash_index += 1;
       hash_index %= SIZE;           
    }
      return llListInsertList(table, [item], hash_index);
       
    }
     return table;
     
    
}
    
list init_table(list table_to_start)
{
   
    list alist = table_to_start;
    integer i = 0;
    while (i < SIZE) 
     {
       alist = llListInsertList(alist, [NULL_STRING], i);
       i++;
     } 
    
    return alist; 
    
}


debug(string say) { 
    if (DEBUG == TRUE)
    {
     llOwnerSay(say);   
    }
}

integer avatar_frequency_index(key avatar_id)
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
    integer index = avatar_frequency_index(avatar_id);
    if (index == -1)
    {
        avatar_frequency += [avatar_id, avatar_name, 1];
    }
    else
    {
        avatar_frequency = llListReplaceList(avatar_frequency,
            [avatar_name, llList2Integer(avatar_frequency, index + 2) + 1], index + 1, index + 2);
    }
    avatar_total_encounters += 1;
}

string avatar_frequency_payload()
{
    list records = [];
    integer i;
    for (i = 0; i < llGetListLength(avatar_frequency); i += 3)
    {
        string record = llList2Json(JSON_OBJECT, [
            "uuid", (string)llList2Key(avatar_frequency, i),
            "name", llList2String(avatar_frequency, i + 1),
            "count", llList2Integer(avatar_frequency, i + 2)
        ]);
        records += record;
    }
    string payload = llList2Json(JSON_OBJECT, [
        "source", "lsl_avatar_sensor",
        "captured_by", llGetUsername(llGetOwner()),
        "sim_name", llGetRegionName(),
        "scan_at", llGetUnixTime(),
        "scan_count", avatar_scan_count,
        "scan_interval_seconds", avatar_scan_interval,
        "total_encounters", avatar_total_encounters,
        "avatars_seen", llGetListLength(avatar_frequency) / 3,
        "avatars", "[]"
    ]);
    payload = str_replace(payload, "\"avatars\":\"[]\"",
        "\"avatars\":[" + llDumpList2String(records, ",") + "]");
    return payload;
}

upload_avatar_frequency()
{
    key request_id = llHTTPRequest(avatar_frequency_url,
        [HTTP_METHOD, "POST", HTTP_MIMETYPE, "application/json"],
        avatar_frequency_payload());
    if (request_id == NULL_KEY)
        llOwnerSay("Avatar frequency upload could not be queued.");
}



check_table(list data_items)
{
    integer partition_size = llGetListLength(data_items);
    if (partition_size >= MAX_STACK_SIZE)
        {
         admin_say("partition_size: " + (string)partition_size);
         admin_say("MAX_STACK_SIZE: " + (string)MAX_STACK_SIZE);   
            
        
        push_to_server(data_items); 
                     data_items = [];
                     debug("max size reached, pushing to server");
                     
                    }
                

    
}

default
{
     

    on_rez(integer start_paramater)
    {   llOwnerSay(":: Rez incoming... Resetting HUDlink systems... ::");
        llResetScript();
        
        
        
    
    }



    state_entry()
    { 
    benchmark_startup_time_start = time();
    if (OUTPUT_BOOTUP){ 
        llWhisper(0, ":: Startup Invoked by self... ::");
        llWhisper(0, ":: Invoke Sequence Initialized... ::");
        llWhisper(0, ":: Bootup Initialized... ::");
        llWhisper(0, ":: Bootstrapping (loading "+ SMALL_VERSION +")");// HUDlink OS v4.1-alpha... ::");   
        llWhisper(0, ":: OK. ::");
        llWhisper(0, ":: VERSION: "+ VERSION + " ::");
        llWhisper(0, ":: Initializing data_items... ::");
        data_items = init_table(data_items);     
        llWhisper(0, ":: OK. Table Initialized. ::");
        llWhisper(0, ":: Setting Chat Handles... ::");
        chat_handle = llListen(channel, "", NULL_KEY, "");
        code_channel = llListen(code_channel, "", llGetOwner(), "");
        llWhisper(0, ":: OK. Chat Handles Set. ::");
        llWhisper(0, ":: HUDlink(v6) Systems Are Active... ::");
        benchmark_startup_time_end = time();
        llWhisper(0, ":: Startup Time: " + (string)(calculate_delta(benchmark_startup_time_end, benchmark_startup_time_start)) + "ms ::");
         llWhisper(0, ":: Memory used at startup: " + ((string)bytes_to_kilobytes(llGetUsedMemory()))+"KB");
         llWhisper(0, ":: Memory free at startup: " + ((string)bytes_to_kilobytes(llGetFreeMemory()))+"KB");
      //  llWhisper(0, ":: % Memory Remaining: " + (string)bytes_to_kilobytes(1.0 - (llGetUsedMemory()/llGetFreeMemory)+"KB");
        }
        else
        {
        llOwnerSay(":: Startup Invoked by self... ::");
        llOwnerSay(":: Invoke Sequence Initialized... ::");
        llOwnerSay(":: Bootup Initialized... ::");
        llOwnerSay(":: Bootstrapping (loading "+ SMALL_VERSION +")");// HUDlink OS v4.1-alpha... ::");   
        llOwnerSay(":: OK. ::");
        llOwnerSay(":: VERSION: "+ VERSION + " ::");
        llOwnerSay(":: Initializing data_items... ::");
        data_items = init_table(data_items);     
        llOwnerSay(":: OK. Table Initialized. ::");
        llOwnerSay(":: Setting Chat Handles... ::");
        chat_handle = llListen(channel, "", NULL_KEY, "");
       // llWhisper(0, ":: OK. Chat Handles Set. ::");
        code_channel = llListen(code_channel, "", llGetOwner(), "");
        llOwnerSay(":: OK. Chat Handles Set. ::");
        llOwnerSay(":: HUDlink(v6.0.1) Systems Are Active... ::");
        benchmark_startup_time_end = time();
        llOwnerSay(":: Startup Time: " + (string)(calculate_delta(benchmark_startup_time_end, benchmark_startup_time_start)) + "ms ::");
         llOwnerSay(":: Memory used at startup: " + ((string)bytes_to_kilobytes(llGetUsedMemory()))+"KB");
         llOwnerSay(":: Memory free at startup: " + ((string)bytes_to_kilobytes(llGetFreeMemory()))+"KB"); 
        //llOwnerSay(":: Startup Time: " + (string)(benchmark_startup_time_end - benchmark_startup_time_start) + " seconds ::");
           //1234
              llSetTimerEvent(avatar_scan_interval);
              llSensor("", NULL_KEY, AGENT, 96.0, PI);
            }
            
        
     
     } 
     
     http_response(key request_id, integer status, list metadata, string body)
       {  
      
       
       //These next two code lines should be hardcoded.
       
     //  llOwnerSay("body: " + body);
      // llOwnerSay("Server response: " + (string)status);
     // debug("return body: " + body);
       //debug("Server response: " + (string)status);
         admin_say("body: " + body);
         admin_say("server response: " + (string)status);
         if (body != "no_additional_data")
         {
             if (URL_PARSING && body != NULL_STRING && body != "<html>
<head><title>502 Bad Gateway</title></head>
<body>
<center><h1>502 Bad Gateway</h1></center>
<hr><center>nginx/1.18.0 (Ubuntu)</center>
</body>
</html>" )
            if (URL_PARSING_PUBLIC)
                llWhisper(0, unescape(body)+"\n");
             else
               llOwnerSay(unescape(body)+"\n");
             }
             
           if (status != 200)
               llOwnerSay("Avatar frequency upload failed with HTTP " + (string)status + ".");
         }
         
         
       
   
     sensor(integer num)
     {
        integer i;
        avatar_scan_count += 1;
        avatar_last_scan_at = llGetUnixTime();
        for (i = 0; i < num; i++)
        {
            record_avatar_encounter(llDetectedKey(i), llDetectedName(i));
        }
        upload_avatar_frequency();
     }

     no_sensor()
     {
        avatar_scan_count += 1;
        avatar_last_scan_at = llGetUnixTime();
        upload_avatar_frequency();
     }

     timer()
     {
        llSensor("", NULL_KEY, AGENT, 96.0, PI);
     }
    
    
    
}