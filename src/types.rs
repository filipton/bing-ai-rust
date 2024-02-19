use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoot {
    pub conversation_id: Option<String>,
    pub client_id: Option<String>,
    pub result: Option<CreateResult>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult {
    pub value: Option<String>,
    pub message: Option<Value>,
}

pub enum Tone {
    Precise,
    Creative,
    Balanced,
}

impl Tone {
    pub fn to_options_set(&self) -> Vec<&str> {
        match self {
            Self::Precise => vec!["h3precise", "clgalileo"],
            Self::Creative => vec!["h3imaginative", "clgalileo", "gencontentv3"],
            Self::Balanced => vec!["galileo"],
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::Precise => "Precise",
            Self::Creative => "Creative",
            Self::Balanced => "Balanced",
        }
    }
}

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

pub fn construct_ask_args(
    prompt: &str,
    invocation_id: i64,
    tone: Tone,
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
