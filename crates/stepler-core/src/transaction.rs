use crate::types::CorrectionMode;
use std::collections::HashSet;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationState {
    Idle,
    HotkeyReceived,
    ContextCaptured,
    PlanBuilt,
    PreflightChecked,
    ReplacementApplied,
    Verified,
    RolledBackOrFailed,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionError {
    InvalidTransition,
    AlreadyTerminal,
    OperationAlreadyActive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTiming {
    pub state: OperationState,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationMetrics {
    pub duration_ms: u128,
    pub timings: Vec<StageTiming>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    operation_id: String,
    mode: CorrectionMode,
    state: OperationState,
    replacement_applied: bool,
    started_at: Instant,
    last_transition_at: Instant,
    timings: Vec<StageTiming>,
}

impl Transaction {
    pub fn new(operation_id: impl Into<String>, mode: CorrectionMode) -> Self {
        let now = Instant::now();
        Self {
            operation_id: operation_id.into(),
            mode,
            state: OperationState::Idle,
            replacement_applied: false,
            started_at: now,
            last_transition_at: now,
            timings: Vec::new(),
        }
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn mode(&self) -> CorrectionMode {
        self.mode
    }

    pub fn state(&self) -> OperationState {
        self.state
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            OperationState::Completed | OperationState::RolledBackOrFailed
        )
    }

    pub fn transition_to(&mut self, next: OperationState) -> Result<(), TransactionError> {
        if self.is_terminal() {
            return Err(TransactionError::AlreadyTerminal);
        }

        if !is_valid_transition(self.state, next) {
            return Err(TransactionError::InvalidTransition);
        }

        if next == OperationState::ReplacementApplied {
            if self.replacement_applied {
                return Err(TransactionError::InvalidTransition);
            }
            self.replacement_applied = true;
        }

        let now = Instant::now();
        self.timings.push(StageTiming {
            state: next,
            elapsed_ms: now.duration_since(self.last_transition_at).as_millis(),
        });
        self.last_transition_at = now;
        self.state = next;
        Ok(())
    }

    pub fn fail(&mut self) {
        if !self.is_terminal() {
            let _ = self.transition_to(OperationState::RolledBackOrFailed);
        }
    }

    pub fn metrics(&self) -> OperationMetrics {
        OperationMetrics {
            duration_ms: Instant::now().duration_since(self.started_at).as_millis(),
            timings: self.timings.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OperationGate {
    active_controls: HashSet<String>,
}

impl OperationGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_acquire(&mut self, control_key: impl Into<String>) -> Result<(), TransactionError> {
        let control_key = control_key.into();
        if self.active_controls.contains(&control_key) {
            return Err(TransactionError::OperationAlreadyActive);
        }

        self.active_controls.insert(control_key);
        Ok(())
    }

    pub fn release(&mut self, control_key: &str) {
        self.active_controls.remove(control_key);
    }

    pub fn is_active(&self, control_key: &str) -> bool {
        self.active_controls.contains(control_key)
    }
}

fn is_valid_transition(current: OperationState, next: OperationState) -> bool {
    use OperationState::*;

    matches!(
        (current, next),
        (Idle, HotkeyReceived)
            | (HotkeyReceived, ContextCaptured)
            | (ContextCaptured, PlanBuilt)
            | (PlanBuilt, PreflightChecked)
            | (PreflightChecked, ReplacementApplied)
            | (ReplacementApplied, Verified)
            | (Verified, Completed)
            | (HotkeyReceived, RolledBackOrFailed)
            | (ContextCaptured, RolledBackOrFailed)
            | (PlanBuilt, RolledBackOrFailed)
            | (PreflightChecked, RolledBackOrFailed)
            | (ReplacementApplied, RolledBackOrFailed)
            | (Verified, RolledBackOrFailed)
    )
}
