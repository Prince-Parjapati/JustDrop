//! Transfer manager orchestrating the complete send/receive flow.
//!
//! Coordinates discovery → handshake → negotiate → chunk stream → verify,
//! with progress reporting and resume support.

use crate::codec::ProtocolCodec;
use crate::messages::Message;
use justdrop_core::config::Config;
use justdrop_core::error::ProtocolError;
use justdrop_core::types::*;
use justdrop_network::{connect, SecureTransport};
use justdrop_security::{IdentityKeys, NoiseSession};
use justdrop_storage::{
    hash_file, ChunkWriter, FileChunker, ResumeManager, TransferResumeState,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Events emitted by the transfer manager.
#[derive(Debug, Clone)]
pub enum TransferEvent {
    /// An incoming transfer request needs user approval.
    IncomingRequest {
        transfer_id: TransferId,
        manifest: TransferManifest,
        peer_name: String,
    },
    /// Transfer progress update.
    Progress(TransferProgress),
    /// Transfer completed successfully.
    Completed {
        transfer_id: TransferId,
        direction: TransferDirection,
    },
    /// Transfer failed.
    Failed {
        transfer_id: TransferId,
        error: String,
    },
    /// Transfer was cancelled.
    Cancelled { transfer_id: TransferId },
}

/// Response to an incoming transfer request.
#[derive(Debug, Clone)]
pub enum IncomingTransferDecision {
    Accept,
    Reject(String),
}

// ─── Handshake helpers ───

/// Perform Noise_XX handshake as initiator over raw TCP, returning a ProtocolCodec.
async fn handshake_initiator(
    mut stream: TcpStream,
    identity: &IdentityKeys,
) -> Result<ProtocolCodec, ProtocolError> {
    let params: snow::params::NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2s"
        .parse()
        .map_err(|e| ProtocolError::Serialization(format!("noise params: {e}")))?;

    let mut hs = snow::Builder::new(params)
        .local_private_key(identity.private_key())
        .map_err(|e| ProtocolError::Serialization(format!("noise key: {e}")))?
        .build_initiator()
        .map_err(|e| ProtocolError::Serialization(format!("noise build: {e}")))?;

    let mut buf = vec![0u8; 65535];
    let mut msg_buf = vec![0u8; 65535];

    // Message 1: → e
    let len = hs
        .write_message(&[], &mut buf)
        .map_err(|e| ProtocolError::Serialization(format!("hs msg1: {e}")))?;
    write_raw_frame(&mut stream, &buf[..len]).await?;

    // Message 2: ← e, ee, s, es
    let msg2 = read_raw_frame(&mut stream).await?;
    hs.read_message(&msg2, &mut msg_buf)
        .map_err(|e| ProtocolError::Deserialization(format!("hs msg2: {e}")))?;

    // Message 3: → s, se
    let len = hs
        .write_message(&[], &mut buf)
        .map_err(|e| ProtocolError::Serialization(format!("hs msg3: {e}")))?;
    write_raw_frame(&mut stream, &buf[..len]).await?;

    let noise_transport = hs
        .into_transport_mode()
        .map_err(|e| ProtocolError::Serialization(format!("hs transport: {e}")))?;

    let session = NoiseSession::new(noise_transport);
    let secure = SecureTransport::new(stream, session);
    info!("handshake complete (initiator)");
    Ok(ProtocolCodec::new(secure))
}

/// Perform Noise_XX handshake as responder over raw TCP, returning a ProtocolCodec.
async fn handshake_responder(
    mut stream: TcpStream,
    identity: &IdentityKeys,
) -> Result<ProtocolCodec, ProtocolError> {
    let params: snow::params::NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2s"
        .parse()
        .map_err(|e| ProtocolError::Serialization(format!("noise params: {e}")))?;

    let mut hs = snow::Builder::new(params)
        .local_private_key(identity.private_key())
        .map_err(|e| ProtocolError::Serialization(format!("noise key: {e}")))?
        .build_responder()
        .map_err(|e| ProtocolError::Serialization(format!("noise build: {e}")))?;

    let mut buf = vec![0u8; 65535];
    let mut msg_buf = vec![0u8; 65535];

    // Message 1: ← e
    let msg1 = read_raw_frame(&mut stream).await?;
    hs.read_message(&msg1, &mut msg_buf)
        .map_err(|e| ProtocolError::Deserialization(format!("hs msg1: {e}")))?;

    // Message 2: → e, ee, s, es
    let len = hs
        .write_message(&[], &mut buf)
        .map_err(|e| ProtocolError::Serialization(format!("hs msg2: {e}")))?;
    write_raw_frame(&mut stream, &buf[..len]).await?;

    // Message 3: ← s, se
    let msg3 = read_raw_frame(&mut stream).await?;
    hs.read_message(&msg3, &mut msg_buf)
        .map_err(|e| ProtocolError::Deserialization(format!("hs msg3: {e}")))?;

    let noise_transport = hs
        .into_transport_mode()
        .map_err(|e| ProtocolError::Serialization(format!("hs transport: {e}")))?;

    let session = NoiseSession::new(noise_transport);
    let secure = SecureTransport::new(stream, session);
    info!("handshake complete (responder)");
    Ok(ProtocolCodec::new(secure))
}

/// Write a length-prefixed raw frame to a TCP stream.
async fn write_raw_frame(stream: &mut TcpStream, data: &[u8]) -> Result<(), ProtocolError> {
    let len = data.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|e| ProtocolError::Serialization(format!("frame write: {e}")))?;
    stream
        .write_all(data)
        .await
        .map_err(|e| ProtocolError::Serialization(format!("data write: {e}")))?;
    Ok(())
}

/// Read a length-prefixed raw frame from a TCP stream.
async fn read_raw_frame(stream: &mut TcpStream) -> Result<Vec<u8>, ProtocolError> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| ProtocolError::Deserialization(format!("frame read: {e}")))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge {
            size: len,
            max: MAX_MESSAGE_SIZE,
        });
    }
    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .map_err(|e| ProtocolError::Deserialization(format!("data read: {e}")))?;
    Ok(buf)
}

// ─── Send Transfer ───

/// Manages outgoing file transfers (sender side).
pub struct SendTransfer {
    config: Config,
    identity: Arc<IdentityKeys>,
}

impl SendTransfer {
    pub fn new(config: Config, identity: Arc<IdentityKeys>) -> Self {
        Self { config, identity }
    }

    /// Send files to a peer.
    pub async fn send_files(
        &self,
        peer: &DeviceInfo,
        file_paths: &[PathBuf],
        event_tx: mpsc::Sender<TransferEvent>,
    ) -> Result<TransferId, ProtocolError> {
        // 1. Build the manifest
        info!(peer = %peer.name, files = file_paths.len(), "preparing transfer");
        let chunk_size = self.config.transfer.chunk_size;
        let mut file_entries = Vec::with_capacity(file_paths.len());

        for (i, path) in file_paths.iter().enumerate() {
            let metadata = tokio::fs::metadata(path)
                .await
                .map_err(|e| ProtocolError::Serialization(format!("file metadata: {e}")))?;

            let sha256 = hash_file(path)
                .await
                .map_err(|e| ProtocolError::Serialization(format!("file hash: {e}")))?;

            let mime = mime_from_path(path);

            file_entries.push(FileEntry {
                index: i as u32,
                relative_path: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string(),
                size: metadata.len(),
                sha256,
                mime_type: mime,
            });
        }

        let manifest = TransferManifest::new(file_entries, self.config.device_name(), chunk_size);
        let transfer_id = manifest.transfer_id;
        let total_bytes = manifest.total_size;

        // 2. Connect to peer
        let stream = connect(peer.addr, &self.config.network)
            .await
            .map_err(|e| ProtocolError::Serialization(format!("connect: {e}")))?;

        // 3. Perform Noise handshake
        let mut codec = handshake_initiator(stream, &self.identity).await?;

        // 4. Send transfer request
        info!(transfer_id = %transfer_id, "sending transfer request");
        codec
            .send(&Message::TransferRequest(manifest.clone()))
            .await?;

        // 5. Wait for response
        let response = codec.recv().await?;
        let skip_chunks: HashSet<u64> = match response {
            Some(Message::TransferResponse(TransferResponse::Accept)) => {
                info!("transfer accepted");
                HashSet::new()
            }
            Some(Message::TransferResponse(TransferResponse::ResumeAt { received_chunks })) => {
                let total_chunks = manifest.total_chunks();
                let completed =
                    ResumeManager::decode_received_bitmap(&received_chunks, total_chunks);
                info!(already_received = completed.len(), "resuming transfer");
                completed
            }
            Some(Message::TransferResponse(TransferResponse::Reject(reason))) => {
                return Err(ProtocolError::Rejected { reason });
            }
            other => {
                return Err(ProtocolError::UnexpectedMessage {
                    state: "Negotiating".into(),
                    tag: format!("{other:?}"),
                });
            }
        };

        // 6. Stream chunks
        let mut bytes_sent: u64 = 0;
        let start_time = Instant::now();

        for (file_idx, (entry, path)) in manifest.files.iter().zip(file_paths.iter()).enumerate() {
            let effective_cs = effective_chunk_size(entry.size, chunk_size);
            let chunker = FileChunker::open(path, file_idx as u32, effective_cs)
                .await
                .map_err(|e| ProtocolError::Serialization(format!("chunker: {e}")))?;

            for chunk_offset in 0..chunker.chunk_count() {
                let global_id = ChunkId {
                    file_index: file_idx as u32,
                    chunk_offset,
                };

                let global_offset = global_chunk_offset(&manifest, file_idx as u32, chunk_offset);
                if skip_chunks.contains(&global_offset) {
                    bytes_sent += effective_cs as u64;
                    continue;
                }

                let chunk = chunker
                    .read_chunk(chunk_offset)
                    .await
                    .map_err(|e| ProtocolError::Serialization(format!("read chunk: {e}")))?;

                codec
                    .send(&Message::ChunkData {
                        id: global_id,
                        data: chunk.data.to_vec(),
                    })
                    .await?;

                // Wait for ACK
                match codec.recv().await? {
                    Some(Message::ChunkAck(ack)) => {
                        if ack.sha256 != chunk.sha256 {
                            warn!(chunk_offset, "chunk SHA mismatch");
                        }
                    }
                    Some(Message::Cancel { reason }) => {
                        return Err(ProtocolError::Cancelled { reason });
                    }
                    other => {
                        return Err(ProtocolError::UnexpectedMessage {
                            state: "Transferring".into(),
                            tag: format!("{other:?}"),
                        });
                    }
                }

                bytes_sent += chunk.data.len() as u64;

                // Emit progress
                let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                let speed_bps = (bytes_sent as f64 / elapsed) as u64;
                let remaining = total_bytes.saturating_sub(bytes_sent);
                let eta = if speed_bps > 0 {
                    Some(remaining / speed_bps)
                } else {
                    None
                };

                let _ = event_tx
                    .send(TransferEvent::Progress(TransferProgress {
                        transfer_id,
                        state: TransferState::Transferring,
                        bytes_transferred: bytes_sent,
                        total_bytes,
                        current_file_index: file_idx as u32,
                        total_files: manifest.files.len() as u32,
                        speed_bps,
                        eta_secs: eta,
                    }))
                    .await;
            }
        }

        // 7. Send completion
        let manifest_hash = compute_manifest_hash(&manifest);
        codec
            .send(&Message::TransferComplete { manifest_hash })
            .await?;

        // 8. Wait for verification
        match codec.recv().await? {
            Some(Message::TransferVerified { ok: true, .. }) => {
                info!(transfer_id = %transfer_id, "transfer verified by receiver");
                let _ = event_tx
                    .send(TransferEvent::Completed {
                        transfer_id,
                        direction: TransferDirection::Sending,
                    })
                    .await;
            }
            Some(Message::TransferVerified {
                ok: false, error, ..
            }) => {
                let msg = error.unwrap_or_else(|| "verification failed".into());
                error!(transfer_id = %transfer_id, error = %msg, "verification failed");
                return Err(ProtocolError::Cancelled { reason: msg });
            }
            other => {
                return Err(ProtocolError::UnexpectedMessage {
                    state: "Verifying".into(),
                    tag: format!("{other:?}"),
                });
            }
        }

        codec.shutdown().await?;
        Ok(transfer_id)
    }
}

// ─── Receive Transfer ───

/// Manages incoming file transfers (receiver side).
pub struct RecvTransfer {
    config: Config,
    identity: Arc<IdentityKeys>,
    resume_manager: ResumeManager,
}

impl RecvTransfer {
    pub fn new(config: Config, identity: Arc<IdentityKeys>) -> Self {
        let state_dir = Config::data_dir().join("resume");
        Self {
            config,
            identity,
            resume_manager: ResumeManager::new(&state_dir),
        }
    }

    /// Handle an incoming connection.
    pub async fn handle_incoming(
        &self,
        stream: TcpStream,
        decision_rx: mpsc::Receiver<IncomingTransferDecision>,
        event_tx: mpsc::Sender<TransferEvent>,
    ) -> Result<(), ProtocolError> {
        let mut decision_rx = decision_rx;

        // 1. Handshake
        let mut codec = handshake_responder(stream, &self.identity).await?;

        // 2. Receive transfer request
        let manifest = match codec.recv().await? {
            Some(Message::TransferRequest(m)) => m,
            other => {
                return Err(ProtocolError::UnexpectedMessage {
                    state: "Negotiating".into(),
                    tag: format!("{other:?}"),
                });
            }
        };

        let transfer_id = manifest.transfer_id;
        info!(
            transfer_id = %transfer_id,
            sender = %manifest.sender_name,
            files = manifest.files.len(),
            total_size = %format_bytes(manifest.total_size),
            "incoming transfer request"
        );

        // 3. Emit event for user decision
        let _ = event_tx
            .send(TransferEvent::IncomingRequest {
                transfer_id,
                manifest: manifest.clone(),
                peer_name: manifest.sender_name.clone(),
            })
            .await;

        let resume_state = self.resume_manager.load(transfer_id).await.ok().flatten();

        // 4. Wait for decision
        let decision = if self.config.security.auto_accept_all {
            IncomingTransferDecision::Accept
        } else {
            match decision_rx.recv().await {
                Some(d) => d,
                None => IncomingTransferDecision::Reject("no response".into()),
            }
        };

        match decision {
            IncomingTransferDecision::Reject(reason) => {
                codec
                    .send(&Message::TransferResponse(TransferResponse::Reject(
                        reason.clone(),
                    )))
                    .await?;
                return Err(ProtocolError::Rejected { reason });
            }
            IncomingTransferDecision::Accept => {
                if let Some(ref state) = resume_state {
                    let total_chunks = manifest.total_chunks();
                    let mut all_completed: HashSet<u64> = HashSet::new();
                    for file_state in &state.files {
                        for &chunk in &file_state.completed_chunks {
                            let global =
                                global_chunk_offset(&manifest, file_state.file_index, chunk);
                            all_completed.insert(global);
                        }
                    }
                    let bitmap =
                        ResumeManager::encode_received_bitmap(&all_completed, total_chunks);
                    codec
                        .send(&Message::TransferResponse(TransferResponse::ResumeAt {
                            received_chunks: bitmap,
                        }))
                        .await?;
                } else {
                    codec
                        .send(&Message::TransferResponse(TransferResponse::Accept))
                        .await?;
                }
            }
        }

        // 5. Receive chunks
        let download_dir = self.config.download_dir();
        let mut writers: Vec<Option<ChunkWriter>> = Vec::new();

        for entry in &manifest.files {
            let dest = download_dir.join(&entry.relative_path);
            let writer = ChunkWriter::new(
                transfer_id,
                entry.index,
                &dest,
                manifest.chunk_size,
                entry.size,
            )
            .await
            .map_err(|e| ProtocolError::Serialization(format!("writer init: {e}")))?;
            writers.push(Some(writer));
        }

        let mut bytes_received: u64 = resume_state
            .as_ref()
            .map(|s| s.bytes_transferred)
            .unwrap_or(0);
        let total_bytes = manifest.total_size;
        let start_time = Instant::now();

        loop {
            match codec.recv().await? {
                Some(Message::ChunkData { id, data }) => {
                    let writer = writers
                        .get_mut(id.file_index as usize)
                        .and_then(|w| w.as_mut())
                        .ok_or_else(|| ProtocolError::Serialization("invalid file index".into()))?;

                    let sha256 = writer
                        .write_chunk(id.chunk_offset, &data)
                        .await
                        .map_err(|e| ProtocolError::Serialization(format!("write: {e}")))?;

                    codec
                        .send(&Message::ChunkAck(ChunkAck { id, sha256 }))
                        .await?;

                    bytes_received += data.len() as u64;

                    let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                    let speed_bps = (bytes_received as f64 / elapsed) as u64;
                    let remaining = total_bytes.saturating_sub(bytes_received);
                    let eta = if speed_bps > 0 {
                        Some(remaining / speed_bps)
                    } else {
                        None
                    };

                    let _ = event_tx
                        .send(TransferEvent::Progress(TransferProgress {
                            transfer_id,
                            state: TransferState::Transferring,
                            bytes_transferred: bytes_received,
                            total_bytes,
                            current_file_index: id.file_index,
                            total_files: manifest.files.len() as u32,
                            speed_bps,
                            eta_secs: eta,
                        }))
                        .await;
                }
                Some(Message::TransferComplete { manifest_hash }) => {
                    info!(transfer_id = %transfer_id, "all chunks received, verifying");

                    let expected_hash = compute_manifest_hash(&manifest);
                    if manifest_hash != expected_hash {
                        warn!("manifest hash mismatch");
                    }

                    let mut all_ok = true;
                    for (i, _entry) in manifest.files.iter().enumerate() {
                        if let Some(writer) = writers[i].take() {
                            if !writer.is_complete() {
                                warn!(file_index = i, "file incomplete");
                                all_ok = false;
                                continue;
                            }
                            writer.finalize().await.map_err(|e| {
                                ProtocolError::Serialization(format!("finalize: {e}"))
                            })?;
                        }
                    }

                    codec
                        .send(&Message::TransferVerified {
                            ok: all_ok,
                            error: if all_ok {
                                None
                            } else {
                                Some("some files incomplete".into())
                            },
                        })
                        .await?;

                    if all_ok {
                        let _ = self.resume_manager.remove(transfer_id).await;
                        let _ = event_tx
                            .send(TransferEvent::Completed {
                                transfer_id,
                                direction: TransferDirection::Receiving,
                            })
                            .await;
                    }
                    break;
                }
                Some(Message::Cancel { reason }) => {
                    warn!(transfer_id = %transfer_id, reason = %reason, "cancelled by sender");
                    self.save_resume_state(transfer_id, &manifest, &writers, bytes_received)
                        .await;
                    let _ = event_tx
                        .send(TransferEvent::Cancelled { transfer_id })
                        .await;
                    return Err(ProtocolError::Cancelled { reason });
                }
                None => {
                    warn!(transfer_id = %transfer_id, "connection closed");
                    self.save_resume_state(transfer_id, &manifest, &writers, bytes_received)
                        .await;
                    return Err(ProtocolError::Cancelled {
                        reason: "connection closed".into(),
                    });
                }
                other => {
                    return Err(ProtocolError::UnexpectedMessage {
                        state: "Transferring".into(),
                        tag: format!("{other:?}"),
                    });
                }
            }
        }

        codec.shutdown().await?;
        Ok(())
    }

    async fn save_resume_state(
        &self,
        transfer_id: TransferId,
        manifest: &TransferManifest,
        writers: &[Option<ChunkWriter>],
        bytes_transferred: u64,
    ) {
        let _ = self.resume_manager.init().await;

        let files: Vec<_> = writers
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                w.as_ref()
                    .map(|writer| justdrop_storage::resume::FileResumeState {
                        file_index: i as u32,
                        dest_path: manifest.files[i].relative_path.clone(),
                        temp_path: String::new(),
                        file_size: manifest.files[i].size,
                        chunk_size: manifest.chunk_size,
                        completed_chunks: writer.completed_chunks().clone(),
                    })
            })
            .collect();

        let state = TransferResumeState {
            transfer_id,
            last_updated: chrono::Utc::now(),
            files,
            bytes_transferred,
            total_bytes: manifest.total_size,
        };

        if let Err(e) = self.resume_manager.save(&state).await {
            error!(error = %e, "failed to save resume state");
        }
    }
}

// ─── Helper functions ───

fn global_chunk_offset(manifest: &TransferManifest, file_index: u32, chunk_offset: u64) -> u64 {
    let mut global = 0u64;
    for entry in &manifest.files {
        if entry.index == file_index {
            return global + chunk_offset;
        }
        global += entry.chunk_count(manifest.chunk_size);
    }
    global + chunk_offset
}

fn compute_manifest_hash(manifest: &TransferManifest) -> Sha256Hash {
    use sha2::{Digest, Sha256};
    let data = bincode::serialize(manifest).unwrap_or_default();
    let result = Sha256::digest(&data);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

fn mime_from_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "json" => "application/json",
        "apk" => "application/vnd.android.package-archive",
        "dmg" => "application/x-apple-diskimage",
        _ => "application/octet-stream",
    }
    .to_string()
}
