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

