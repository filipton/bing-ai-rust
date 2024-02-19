use crate::types::Tone;
use serde_json::{json, Value};

const OPTIONS_SETS: [&str; 10] = [
    "nlu_direct_response_filter",
    "deepleo",
    "disable_emoji_spoken_text",
    "responsible_ai_policy_235",
    "enablemm",
    "dv3sugg",
    "iyxapbing",
    "iycapbing",
    "saharagenconv5",
    "eredirecturl",
];

const ALLOWED_MESSAGE_TYPES: [&str; 16] = [
    "Chat",
    "ActionRequest",
    "AdsQuery",
    "ConfirmationCard",
    "Context",
    "Disengaged",
    "InternalLoaderMessage",
    "InternalSearchQuery",
    "InternalSearchResult",
    "InvokeAction",
    "Progress",
    "RenderCardRequest",
    "RenderContentRequest",
    "SemanticSerp",
    "GenerateContentQuery",
    "SearchQuery",
];

const CONVERSATION_HISTORY_OPTIONS_SETS: [&str; 4] =
    ["autosave", "savemem", "uprofupd", "uprofgen"];

pub fn ask_json(
    prompt: &str,
    invocation_id: i64,
    tone: &Tone,
    conversation_signature: &str,
    client_id: &str,
    conversation_id: &str,
) -> Value {
    let mut options_sets = OPTIONS_SETS.to_vec();
    options_sets.extend(tone.to_options_set());

    json!({
        "arguments": [
            {
              "source": "cib",
              "optionsSets": options_sets,
              "allowedMessageTypes": ALLOWED_MESSAGE_TYPES,
              "sliceIds": [],
              "verbosity": "verbose",
              "scenario": "SERP",
              "plugins": [],
              "conversationHistoryOptionsSets": CONVERSATION_HISTORY_OPTIONS_SETS,
              "isStartOfSession": invocation_id == 0,
              "message": {
                "author": "user",
                "inputMethod": "Keyboard",
                "text": prompt,
                "messageType": "Chat",
                "imageUrl": null,
                "originalImageUrl": null
              },
              "conversationSignature": conversation_signature,
              "participant": {
                "id": client_id
              },
              "tone": tone.to_str(),
              "spokenTextMode": "None",
              "conversationId": conversation_id
            }
          ],
          "invocationId": invocation_id.to_string(),
          "target": "chat",
          "type": 4
    })
}
