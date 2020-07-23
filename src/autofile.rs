use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct AutoFile {
    pub tasks: HashMap<String, Task>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    /// Executable to start
    pub program: String,

    /// Additional arguments to pass to program
    #[serde(default)]
    pub arguments: Vec<String>,

    /// Which tasks need to run before this task can be run in turn
    #[serde(default)]
    pub needs: Vec<String>,
}
