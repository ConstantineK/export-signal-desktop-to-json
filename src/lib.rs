use log::trace;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

// Lets fix the attachment parts -
// I know we have references everywhere, so we need to be able to load all the attachments
// Can we just .. embed them? That's going to fuck em up the HTML conversation.
// Could I load stuff based on the viewport with some javascript magic?
// These problems are fun but really all out of scope for the thing I want to do
// What I should instead do is produce a simple json output that's something anyone could parse into a normal set of messages.

// As well as copying and re-assembling the image folder in a sane way with extensions, if we dont do this the images would be deleted.
// We should do this in an additive way - if someone deletes a message we'd like to keep the image.
// In the future we could "undelete" things that up in the database without existing in the actual stream

pub fn write_conversations_to_json(
    output_folder: PathBuf,
    conversations: Vec<ConversationOutput>,
) -> rusqlite::Result<(), Box<dyn Error>> {
    // Use Rayon to parallelize the processing of conversations
    conversations.par_iter().for_each(|partition| {
        // Clone values for this thread
        let name = partition
            .profile_name
            .clone()
            .unwrap_or_else(|| partition.conversation_id.clone());

        let mut filename = PathBuf::new();
        filename.push(output_folder.clone());
        filename.push(name + ".json");

        // Serialize to a JSON string
        let json_data = to_string_pretty(&partition).unwrap();

        // Use synchronous file operations to write the JSON data to a file
        if let Err(err) = fs::create_dir_all(output_folder.clone()) {
            eprintln!("Error creating directory: {}", err);
        }

        if let Err(err) = fs::write(&filename, json_data) {
            eprintln!("Error writing file {:?}: {}", filename, err);
        } else {
            trace!("Writing file: {:?}", filename);
        }
    });
    Ok(())
}

// I am not currently sending the right data to unwrap_signal_message
fn extract_useful_fields_from_json(message: &str) -> rusqlite::Result<FullMessage, Box<dyn Error>> {
    let parsed: FullMessage = serde_json::from_str(message)?;
    // for now, just send out the timestamp, message, and the attachments
    Ok(parsed)
}

fn extract_raw_messages_to_formatted_messages(
    raw_conversations: Vec<rusqlite::Result<FullMessage>>,
) -> Result<Vec<ConversationOutput>, Box<dyn Error>> {
    // Take the raw data and convert it into a more nested structure

    // Group messages by conversation_id
    let mut conversations: HashMap<String, ConversationOutput> = HashMap::new();

    for message in raw_conversations {
        let msg = message.unwrap();
        // let me do the creation/comparison on one set and then it doesn't complain
        let compare = msg.clone();
        let conversation_id = compare.conversation_id.clone();

        // Create a new NestedData entry if it doesn't exist
        // or get the current copy of the entry and add the message to it

        let entry = conversations
            .entry(conversation_id.clone())
            .or_insert(ConversationOutput {
                profile_name: compare.profile_name.clone(),
                conversation_id: compare.conversation_id.clone(),
                conv_type: compare.r#type.unwrap().clone(),
                e164: compare.e164.clone(),
                messages: Vec::new(),
            });

        // Format the message and push it to the conversation's messages
        if compare.timestamp.is_none() {
            continue;
        }

        if &compare.message_name.unwrap() == "null" {
            continue;
        }

        entry.messages.push(msg);
    }

    for (_, conversation) in conversations.iter_mut() {
        conversation
            .messages
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    // sort the raw_conversations messages by the timestamp
    // Convert the HashMap into a Vec
    let result: Vec<ConversationOutput> = conversations.into_iter().map(|(_, v)| v).collect();

    Ok(result)
}

pub fn get_signal_data_from_sqlite(
    database_file_path: PathBuf,
    signal_key: String,
) -> Result<Vec<ConversationOutput>, Box<dyn Error>> {
    // Right now set the path to the file and the key manually
    // The config json file has a key value that is the password to the database - my computer its "C:\Users\ck\AppData\Roaming\Signal\config.json"
    let db_file = Path::new(&database_file_path);
    let key = String::from(signal_key);
    let conn = Connection::open_with_flags(db_file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Decrypt the signal file
    unlock_sqlite_database_with_pragma(&conn, &key)?;

    // I need to have better error handling when converting messages into data structures
    // if I fail to convert a message I should report that I can't convert it and skip it
    let mut message_stmt = conn.prepare(
        "
SELECT
    m.json,
    m.conversationId,
    c.type,
    cast(c.e164 as bigint) as e164,
    case 
        when c.profileName is null 
        then conversationId 
        else c.profileName 
    end as profileName,
    ifnull(
        case 
            when m.type = 'incoming' 
            then c.profileFullName 
            else 'me' 
        end, 
        'system'
    ) as messageName
FROM messages as m
JOIN conversations as c 
    ON c.id = m.conversationId
ORDER BY m.sent_at;",
    )?;

    trace!("Executing query message query");
    // So the third param is actually a function passed to return stuff

    // this is the wrong thing to return, because I actually need the Conversationoutput, but I should build the message first
    let iterator: Vec<rusqlite::Result<FullMessage>> = message_stmt
        .query_map((), |row| {
            // there's your problem, this isn't the thing that has it
            let json_message: String = row.get(0)?;
            let conv_type: String = row.get(2)?;
            let e164: Option<i64> = row.get(3)?;
            let profile_name: Option<String> = row.get(4)?;
            let message_name: String = row.get(5)?;

            let mut json_fields = match extract_useful_fields_from_json(&json_message) {
                Ok(x) => x,
                Err(err) => {
                    eprintln!("Error parsing json: {}", err);
                    exit(1);
                }
            };

            json_fields.r#type = Option::from(conv_type);
            json_fields.e164 = e164;
            json_fields.profile_name = profile_name;
            json_fields.message_name = Option::from(message_name);

            Ok(json_fields)
            // This either does not run or somehow doesn't print
        })
        .unwrap()
        .collect();
    // I still dont have a top level thing, just the json stuff

    trace!("Query finished, formatting data.");
    let data = extract_raw_messages_to_formatted_messages(iterator);
    trace!("Finished formatting data, returning.");
    Ok(data?)
}

fn unlock_sqlite_database_with_pragma(conn: &Connection, key: &str) -> Result<(), Box<dyn Error>> {
    // This sucks, can I make a type to make this make sense?
    // I know I need content that's just alphanumeric in the input
    // I know it needs to say PRAGMA key = x'"key"';
    let mut pragma_query = "PRAGMA key = \"x'".to_string();
    pragma_query.push_str(&key.replace('"', ""));
    pragma_query.push_str("'\";");

    trace!("Setting pragma: {}", &pragma_query);

    let mut stmt = conn.prepare(&pragma_query)?;
    let _ = stmt.query_and_then::<(), Box<dyn Error>, _, _>((), |row| {
        let result: String = row.get(0)?;
        trace!("Result: {}", result);
        Ok(())
    })?;
    trace!("Pragma set");
    Ok(())
}

pub fn get_signal_key(config_path: PathBuf) -> rusqlite::Result<String, Box<dyn Error>> {
    // We could technically use the signal folder but fuck it

    match fs::metadata(&config_path) {
        Ok(_) => {
            // We got the config file, we return the value of the json key in the config.json file
            match fs::read_to_string(&config_path) {
                Ok(contents) => {
                    let json: serde_json::Value = serde_json::from_str(&contents)?;
                    let signal_key = json["key"].to_string();
                    return Ok(signal_key);
                }
                Err(_) => {
                    eprintln!("{}", "Failed to read config.json within the configured SIGNAL_PATH or default folder. Please copy this file or set the SIGNAL_PATH env variable to the folder containing your config.json.");
                    exit(1);
                }
            }
        }
        Err(_) => {
            eprintln!("{}", "Failed to find config.json within the configured SIGNAL_PATH or default folder. Please copy this file or set the SIGNAL_PATH env variable to the folder containing your config.json.");
            exit(1);
        }
    };
}
//

#[derive(Serialize, Clone)]
pub struct ConversationData {
    timestamp: String,
    body: String,
    conversation_id: String,
    conv_type: String,
    e164: i64,
    profile_name: Option<String>,
    message_name: String,
}

#[derive(Serialize, Clone)]
pub struct Message {
    timestamp_in_ms: i64,
    body: String,
    from: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConversationOutput {
    profile_name: Option<String>,
    conversation_id: String,
    conv_type: String,
    e164: Option<i64>,
    messages: Vec<FullMessage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FullMessage {
    timestamp: Option<i64>,
    attachments: Option<Vec<Attachment>>,
    body: Option<String>,
    #[serde(rename(deserialize = "conversationId", serialize = "conversation_id"))]
    conversation_id: String,
    sent_at: Option<i64>,
    received_at: Option<i64>,
    received_at_ms: Option<i64>,
    recipients: Option<Vec<String>>,
    #[serde(rename(deserialize = "hasAttachments", serialize = "has_attachments"))]
    has_attachments: Option<i32>,
    #[serde(rename(
        deserialize = "hasVisualMediaAttachments",
        serialize = "has_visual_media_attachments"
    ))]
    has_visual_media_attachments: Option<i32>,
    destination: Option<String>,
    from: Option<String>,
    r#type: Option<String>,
    e164: Option<i64>,
    #[serde(rename(deserialize = "profileName", serialize = "profile_name"))]
    profile_name: Option<String>,
    #[serde(rename(deserialize = "messageName", serialize = "message_name"))]
    message_name: Option<String>,
    id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Attachment {
    #[serde(rename(deserialize = "contentType", serialize = "content_type"))]
    content_type: String,
    path: String,
    size: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
    thumbnail: Option<Thumbnail>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Thumbnail {
    path: String,
    #[serde(rename(deserialize = "contentType", serialize = "content_type"))]
    content_type: String,
    width: i32,
    height: i32,
}
