//! Transfer state machine with validated transitions.
//!
//! Enforces the protocol state diagram and prevents invalid state transitions.

use justdrop_core::error::ProtocolError;
use justdrop_core::types::TransferState;
use tracing::{debug, warn};

/// State machine for a single transfer session.
#[derive(Debug)]
pub struct TransferStateMachine {
    state: TransferState,
}

impl TransferStateMachine {
    /// Create a new state machine starting at Handshaking.
    pub fn new() -> Self {
        Self {
            state: TransferState::Handshaking,
        }
    }

    /// Create a state machine starting from a specific state (for resume).
    pub fn from_state(state: TransferState) -> Self {
        Self { state }
    }

    /// Get the current state.
    pub fn state(&self) -> TransferState {
        self.state
    }

    /// Attempt to transition to a new state.
    pub fn transition(&mut self, to: TransferState) -> Result<(), ProtocolError> {
        if self.is_valid_transition(to) {
            debug!(from = %self.state, to = %to, "state transition");
            self.state = to;
            Ok(())
        } else {
            warn!(from = %self.state, to = %to, "invalid state transition");
            Err(ProtocolError::UnexpectedMessage {
                state: self.state.to_string(),
                tag: to.to_string(),
            })
        }
    }

    /// Check if a transition from current state to the target is valid.
    fn is_valid_transition(&self, to: TransferState) -> bool {
        use TransferState::*;

        matches!(
            (self.state, to),
            // Normal flow
            (Handshaking, Negotiating)
                | (Negotiating, Transferring)
                | (Transferring, Verifying)
                | (Verifying, Completed)
                // Rejection
                | (Negotiating, Cancelled)
                // Cancellation from any active state
                | (Handshaking, Cancelled)
                | (Transferring, Cancelled)
                | (Verifying, Cancelled)
                // Failure from any active state
                | (Handshaking, Failed)
                | (Negotiating, Failed)
                | (Transferring, Failed)
                | (Verifying, Failed)
                // Pause/Resume
                | (Transferring, Paused)
                | (Paused, Transferring)
                | (Paused, Cancelled)
                | (Paused, Failed)
                // Resume negotiation
                | (Negotiating, Negotiating) // For ResumeAt
        )
    }

    /// Check if the transfer is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TransferState::Completed | TransferState::Failed | TransferState::Cancelled
        )
    }

    /// Check if the transfer is active (not terminal).
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

impl Default for TransferStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_flow() {
        let mut sm = TransferStateMachine::new();
        assert_eq!(sm.state(), TransferState::Handshaking);

        sm.transition(TransferState::Negotiating).unwrap();
        sm.transition(TransferState::Transferring).unwrap();
        sm.transition(TransferState::Verifying).unwrap();
        sm.transition(TransferState::Completed).unwrap();

        assert!(sm.is_terminal());
    }

    #[test]
    fn rejection_flow() {
        let mut sm = TransferStateMachine::new();
        sm.transition(TransferState::Negotiating).unwrap();
        sm.transition(TransferState::Cancelled).unwrap();
        assert!(sm.is_terminal());
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut sm = TransferStateMachine::new();
        // Can't go from Handshaking directly to Transferring
        assert!(sm.transition(TransferState::Transferring).is_err());
    }

    #[test]
    fn cancel_from_transferring() {
        let mut sm = TransferStateMachine::new();
        sm.transition(TransferState::Negotiating).unwrap();
        sm.transition(TransferState::Transferring).unwrap();
        sm.transition(TransferState::Cancelled).unwrap();
        assert!(sm.is_terminal());
    }

    #[test]
    fn pause_resume_flow() {
        let mut sm = TransferStateMachine::new();
        sm.transition(TransferState::Negotiating).unwrap();
        sm.transition(TransferState::Transferring).unwrap();
        sm.transition(TransferState::Paused).unwrap();
        assert!(sm.is_active());
        sm.transition(TransferState::Transferring).unwrap();
        sm.transition(TransferState::Verifying).unwrap();
        sm.transition(TransferState::Completed).unwrap();
    }
}
