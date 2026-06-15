//! Stress test: concurrent stream transfer.

use justdrop_transport::endpoint::{connect, create_client, create_server, QuicConnection};
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};

fn gen_cert() -> (Vec<u8>, Vec<u8>) {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let params = rcgen::CertificateParams::new(vec!["justdrop".into()]).unwrap();
    let cert = params.self_signed(&key).unwrap();
    (key.serialize_der(), cert.der().to_vec())
}

/// Test 50 sequential streams transferring 64KB each.
#[tokio::test]
async fn concurrent_50_streams() {
    let (key_der, cert_der) = gen_cert();
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = create_server(bind_addr, &key_der, &cert_der).await.unwrap();
    let actual_addr = server.local_addr().unwrap();

    let stream_count = 50usize;
    let payload_size = 64 * 1024;

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let qc = QuicConnection::new(conn);

        for _ in 0..stream_count {
            let (mut send, mut recv) = qc.accept_stream().await.unwrap();
            let frame = recv.read_frame(payload_size + 1024).await.unwrap();
            assert_eq!(frame.len(), payload_size);
            send.write_frame(b"ack").await.unwrap();
            send.finish().await.unwrap();
        }

        let _ = done_rx.await;
    });

    let client_endpoint = create_client().unwrap();
    let client_conn = connect(&client_endpoint, actual_addr).await.unwrap();

    let payload = vec![0xCDu8; payload_size];

    for _ in 0..stream_count {
        let (mut send, mut recv) = client_conn.open_stream().await.unwrap();
        send.write_frame(&payload).await.unwrap();
        send.finish().await.unwrap();
        let resp = recv.read_frame(1024).await.unwrap();
        assert_eq!(&resp[..], b"ack");
    }

    let _ = done_tx.send(());
    timeout(Duration::from_secs(30), server_task)
        .await
        .unwrap()
        .unwrap();
}

/// Test sequential large transfers: 10 × 1MB.
#[tokio::test]
async fn sequential_10mb() {
    let (key_der, cert_der) = gen_cert();
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = create_server(bind_addr, &key_der, &cert_der).await.unwrap();
    let actual_addr = server.local_addr().unwrap();

    let chunk_size = 1024 * 1024;
    let chunk_count = 10;

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let qc = QuicConnection::new(conn);

        for _ in 0..chunk_count {
            let (mut send, mut recv) = qc.accept_stream().await.unwrap();
            let frame = recv.read_frame(chunk_size + 1024).await.unwrap();
            assert_eq!(frame.len(), chunk_size);
            send.write_frame(b"ok").await.unwrap();
            send.finish().await.unwrap();
        }

        let _ = done_rx.await;
    });

    let client_endpoint = create_client().unwrap();
    let client_conn = connect(&client_endpoint, actual_addr).await.unwrap();
    let payload = vec![0xABu8; chunk_size];

    for _ in 0..chunk_count {
        let (mut send, mut recv) = client_conn.open_stream().await.unwrap();
        send.write_frame(&payload).await.unwrap();
        send.finish().await.unwrap();
        let resp = recv.read_frame(1024).await.unwrap();
        assert_eq!(&resp[..], b"ok");
    }

    let _ = done_tx.send(());
    timeout(Duration::from_secs(30), server_task)
        .await
        .unwrap()
        .unwrap();
}
