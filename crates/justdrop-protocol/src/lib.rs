//! # JustDrop Protocol
//!
//! Wire protocol implementation including message serialization, codec framing,
//! transfer state machine, and the full send/receive transfer manager.
//!
//! V2 messages use Postcard encoding for compact binary format.

pub mod codec;
pub mod messages;
pub mod messages_v2;
pub mod state;
pub mod transfer;

pub use codec::ProtocolCodec;
pub use messages::Message;
pub use messages_v2::MessageV2;
pub use state::TransferStateMachine;
pub use transfer::{IncomingTransferDecision, RecvTransfer, SendTransfer, TransferEvent};
