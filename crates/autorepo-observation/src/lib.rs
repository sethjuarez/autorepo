//! Local-first observation contracts for CLI and desktop workflows.

use autorepo::{OperationId, WorkflowRunId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationEvent {
    pub run_id: WorkflowRunId,
    pub operation_id: Option<OperationId>,
    pub kind: ObservationEventKind,
}

pub const ALL_EVENT_KINDS: &[ObservationEventKind] = &[
    ObservationEventKind::PackLoadStarted,
    ObservationEventKind::PackLoadCompleted,
    ObservationEventKind::PackLoadFailed,
    ObservationEventKind::PackValidateStarted,
    ObservationEventKind::PackValidateCompleted,
    ObservationEventKind::PackValidateFailed,
    ObservationEventKind::PlanRenderStarted,
    ObservationEventKind::PlanRenderCompleted,
    ObservationEventKind::PlanRenderFailed,
    ObservationEventKind::ExecutionStarted,
    ObservationEventKind::OperationStarted,
    ObservationEventKind::OperationCompleted,
    ObservationEventKind::OperationFailed,
    ObservationEventKind::ExecutionCompleted,
    ObservationEventKind::GithubRequestStarted,
    ObservationEventKind::GithubRequestCompleted,
    ObservationEventKind::GithubRequestFailed,
    ObservationEventKind::UiConfirmationRequested,
    ObservationEventKind::UiConfirmationAccepted,
    ObservationEventKind::UiConfirmationCancelled,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationEventKind {
    PackLoadStarted,
    PackLoadCompleted,
    PackLoadFailed,
    PackValidateStarted,
    PackValidateCompleted,
    PackValidateFailed,
    PlanRenderStarted,
    PlanRenderCompleted,
    PlanRenderFailed,
    ExecutionStarted,
    OperationStarted,
    OperationCompleted,
    OperationFailed,
    ExecutionCompleted,
    GithubRequestStarted,
    GithubRequestCompleted,
    GithubRequestFailed,
    UiConfirmationRequested,
    UiConfirmationAccepted,
    UiConfirmationCancelled,
}

impl ObservationEventKind {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::PackLoadStarted => "pack.load.started",
            Self::PackLoadCompleted => "pack.load.completed",
            Self::PackLoadFailed => "pack.load.failed",
            Self::PackValidateStarted => "pack.validate.started",
            Self::PackValidateCompleted => "pack.validate.completed",
            Self::PackValidateFailed => "pack.validate.failed",
            Self::PlanRenderStarted => "plan.render.started",
            Self::PlanRenderCompleted => "plan.render.completed",
            Self::PlanRenderFailed => "plan.render.failed",
            Self::ExecutionStarted => "execution.started",
            Self::OperationStarted => "operation.started",
            Self::OperationCompleted => "operation.completed",
            Self::OperationFailed => "operation.failed",
            Self::ExecutionCompleted => "execution.completed",
            Self::GithubRequestStarted => "github.request.started",
            Self::GithubRequestCompleted => "github.request.completed",
            Self::GithubRequestFailed => "github.request.failed",
            Self::UiConfirmationRequested => "ui.confirmation.requested",
            Self::UiConfirmationAccepted => "ui.confirmation.accepted",
            Self::UiConfirmationCancelled => "ui.confirmation.cancelled",
        }
    }
}
