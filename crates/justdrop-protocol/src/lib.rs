//! # JustDrop Protocol
//!
//! Wire protocol implementation including message serialization, codec framing,
//! transfer state machine, and the full send/receive transfer manager.

pub mod codec;
pub mod messages;
pub mod state;
pub mod transfer;

pub use codec::ProtocolCodec;
pub use messages::Message;
pub use state::TransferStateMachine;
pub use transfer::{IncomingTransferDecision, RecvTransfer, SendTransfer, TransferEvent};
