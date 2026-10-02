use serde::{Deserialize, Serialize};

pub const TERMINAL_RECORDING_RECIPE: &str = "terminal-recording";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRecordingRecipePlan {
    pub file_name: String,
    pub recordings: Vec<TerminalRecordingPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRecordingPlan {
    pub media_id: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}
