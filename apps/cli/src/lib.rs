//! Shared library API for the `autorepo` CLI and desktop surfaces.

use serde::{Deserialize, Serialize};

pub mod doctor;
pub mod executor;
pub mod github;
pub mod markers;
pub mod ops;
pub mod pack;
pub mod planner;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationId(pub String);
