//! JustDrop Daemon — background service for file transfer.
//!
//! Starts mDNS discovery, listens for incoming transfers, and provides
//! a CLI for sending files to discovered peers.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use justdrop_core::config::Config;
use justdrop_core::types::format_bytes;
use justdrop_discovery::{PeerEvent, ServiceBrowser, ServiceRegistrar};
use justdrop_network::TransferListener;
use justdrop_protocol::{IncomingTransferDecision, RecvTransfer, SendTransfer, TransferEvent};
use justdrop_security::IdentityKeys;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "justdrop")]
#[command(about = "JustDrop — Cross-platform local file transfer")]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Log level override
    #[arg(short, long)]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon (listen for incoming transfers + discover peers)
    Daemon,

    /// Send files to a peer
    Send {
        /// Peer device name or ID
        #[arg(short, long)]
        peer: String,

        /// Files to send
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },

    /// List discovered peers
    Peers,

    /// Show device identity (fingerprint)
    Identity,

    /// Show status of active/pending transfers
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let log_level = cli.log_level.as_deref().unwrap_or("info");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("justdrop={log_level}").parse().unwrap()),
        )
        .with_target(false)
        .with_thread_ids(true)
        .init();

    // Load config
    let config = Config::load(&cli.config).context("failed to load config")?;
    info!(
        device_name = %config.device_name(),
        port = config.network.listen_port,
        "JustDrop starting"
    );

    // Load or generate identity keys
    let data_dir = Config::data_dir();
    std::fs::create_dir_all(&data_dir).context("failed to create data dir")?;
    let identity = Arc::new(
        IdentityKeys::load_or_generate(&data_dir).context("failed to load identity keys")?,
    );

    match cli.command {
        Commands::Daemon => run_daemon(config, identity).await,
        Commands::Send { peer, files } => run_send(config, identity, &peer, &files).await,
        Commands::Peers => run_peers(config, identity).await,
        Commands::Identity => {
            println!("Device: {}", config.device_name());
            println!("Fingerprint: {}", identity.fingerprint_hex());
            Ok(())
        }
        Commands::Status => {
            println!("No active transfers.");
            Ok(())
        }
    }
}

/// Run the background daemon.
async fn run_daemon(config: Config, identity: Arc<IdentityKeys>) -> Result<()> {
    let service_type = config.discovery.service_type.clone();
    let device_name = config.device_name();
    let port = config.network.listen_port;
    let fingerprint = *identity.fingerprint();

    // Start mDNS registration
    let mut registrar = ServiceRegistrar::new(&service_type, &device_name)
        .context("failed to create mDNS registrar")?;
    registrar
        .register(port, &fingerprint)
        .context("failed to register mDNS service")?;

    // Start mDNS browsing
    let (browser, mut peer_rx) = ServiceBrowser::new(&service_type);
    browser
        .start_browsing(registrar.daemon())
        .context("failed to start mDNS browsing")?;

    // Start TCP listener
    let listener = TransferListener::bind(&config.network)
        .await
        .context("failed to bind TCP listener")?;

    info!(
        addr = %listener.local_addr(),
        fingerprint = %identity.fingerprint_hex(),
        "daemon ready — listening for connections and peers"
    );

    // Event channel
    let (event_tx, mut event_rx) = mpsc::channel::<TransferEvent>(64);

    // Spawn peer event logger
    tokio::spawn(async move {
        loop {
            match peer_rx.recv().await {
                Ok(PeerEvent::Discovered(peer)) => {
                    info!(
                        name = %peer.name,
                        addr = %peer.addr,
                        platform = %peer.platform,
                        "📱 peer discovered"
                    );
                }
                Ok(PeerEvent::Lost(id)) => {
                    info!(id = %id, "peer lost");
                }
                Ok(PeerEvent::Updated(peer)) => {
                    info!(name = %peer.name, "peer updated");
                }
                Err(_) => break,
            }
        }
    });

    // Spawn event logger
    let _event_logger = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                TransferEvent::IncomingRequest {
                    transfer_id,
                    manifest,
                    peer_name,
                } => {
                    info!(
                        transfer_id = %transfer_id,
                        sender = %peer_name,
                        files = manifest.files.len(),
                        size = %format_bytes(manifest.total_size),
                        "📥 incoming transfer request"
                    );
                    for file in &manifest.files {
                        info!("  📄 {} ({})", file.relative_path, format_bytes(file.size));
                    }
                }
                TransferEvent::Progress(p) => {
                    if p.percent() % 10 == 0 {
                        info!(
                            transfer_id = %p.transfer_id,
                            progress = %format!("{}%", p.percent()),
                            speed = %format!("{}/s", format_bytes(p.speed_bps)),
                            eta = ?p.eta_secs.map(|s| format!("{s}s")),
                            "⬇️  progress"
                        );
                    }
                }
                TransferEvent::Completed {
                    transfer_id,
                    direction,
                } => {
                    info!(
                        transfer_id = %transfer_id,
                        direction = ?direction,
                        "✅ transfer complete"
                    );
                }
                TransferEvent::Failed { transfer_id, error } => {
                    error!(transfer_id = %transfer_id, error = %error, "❌ transfer failed");
                }
                TransferEvent::Cancelled { transfer_id } => {
                    warn!(transfer_id = %transfer_id, "🚫 transfer cancelled");
                }
            }
        }
    });

    // Accept incoming connections
    let _recv_handler = RecvTransfer::new(config.clone(), Arc::clone(&identity));

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!(peer = %addr, "incoming connection");

                let (decision_tx, decision_rx) = mpsc::channel(1);
                let event_tx = event_tx.clone();

                // Auto-accept for daemon mode (configurable)
                let auto_accept = config.security.auto_accept_all;
                if auto_accept {
                    let _ = decision_tx.send(IncomingTransferDecision::Accept).await;
                }

                let recv = RecvTransfer::new(config.clone(), Arc::clone(&identity));
                tokio::spawn(async move {
                    if let Err(e) = recv.handle_incoming(stream, decision_rx, event_tx).await {
                        error!(error = %e, "incoming transfer failed");
                    }
                });
            }
            Err(e) => {
                error!(error = %e, "accept failed");
            }
        }
    }
}

/// Send files to a named peer.
async fn run_send(
    config: Config,
    identity: Arc<IdentityKeys>,
    peer_name: &str,
    files: &[PathBuf],
) -> Result<()> {
    let service_type = config.discovery.service_type.clone();
    let device_name = config.device_name();

    // Register and browse
    let mut registrar = ServiceRegistrar::new(&service_type, &device_name)?;
    registrar.register(config.network.listen_port, identity.fingerprint())?;

    let (browser, _) = ServiceBrowser::new(&service_type);
    browser.start_browsing(registrar.daemon())?;

    info!("searching for peer '{peer_name}'...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let peers = browser.peers();
    let peer = peers
        .iter()
        .find(|p| p.name.contains(peer_name) || p.id.contains(peer_name))
        .context(format!(
            "peer '{peer_name}' not found. Available: {:?}",
            peers.iter().map(|p| &p.name).collect::<Vec<_>>()
        ))?;

    info!(
        peer = %peer.name,
        addr = %peer.addr,
        "found peer, starting transfer"
    );

    let sender = SendTransfer::new(config, identity);
    let (event_tx, mut event_rx) = mpsc::channel(64);

    // Progress printer
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                TransferEvent::Progress(p) => {
                    eprint!(
                        "\r  {} {}% ({}/s) ETA: {}s    ",
                        format_bytes(p.bytes_transferred),
                        p.percent(),
                        format_bytes(p.speed_bps),
                        p.eta_secs.unwrap_or(0)
                    );
                }
                TransferEvent::Completed { .. } => {
                    eprintln!("\n✅ Transfer complete!");
                }
                TransferEvent::Failed { error, .. } => {
                    eprintln!("\n❌ Transfer failed: {error}");
                }
                _ => {}
            }
        }
    });

    let file_paths: Vec<PathBuf> = files.to_vec();
    let transfer_id = sender.send_files(peer, &file_paths, event_tx).await?;
    info!(transfer_id = %transfer_id, "transfer complete");

    Ok(())
}

/// List discovered peers.
async fn run_peers(config: Config, identity: Arc<IdentityKeys>) -> Result<()> {
    let service_type = config.discovery.service_type.clone();
    let device_name = config.device_name();

    let mut registrar = ServiceRegistrar::new(&service_type, &device_name)?;
    registrar.register(config.network.listen_port, identity.fingerprint())?;

    let (browser, _) = ServiceBrowser::new(&service_type);
    browser.start_browsing(registrar.daemon())?;

    println!("Searching for peers (5 seconds)...\n");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let peers = browser.peers();
    if peers.is_empty() {
        println!("No peers found.");
    } else {
        println!(
            "{:<20} {:<20} {:<15} ID",
            "NAME", "ADDRESS", "PLATFORM"
        );
        println!("{}", "-".repeat(70));
        for peer in &peers {
            println!(
                "{:<20} {:<20} {:<15} {}",
                peer.name, peer.addr, peer.platform, peer.id
            );
        }
    }

    Ok(())
}
