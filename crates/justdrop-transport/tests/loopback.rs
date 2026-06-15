//! Integration test: QUIC loopback transfer.

use justdrop_transport::endpoint::{connect, create_client, create_server, QuicConnection};
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};

fn gen_test_cert() -> (Vec<u8>, Vec<u8>) {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let params = rcgen::CertificateParams::new(vec!["justdrop".into()]).unwrap();
    let cert = params.self_signed(&key).unwrap();
    (key.serialize_der(), cert.der().to_vec())
}

#[tokio::test]
async fn loopback_framed_transfer() {
    let (key_der, cert_der) = gen_test_cert();

    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = create_server(bind_addr, &key_der, &cert_der).await.unwrap();
    let actual_addr = server.local_addr().unwrap();

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let qc = QuicConnection::new(conn);
        let (mut send, mut recv) = qc.accept_stream().await.unwrap();

        let frame = recv.read_frame(1024).await.unwrap();
        assert_eq!(&frame[..], b"hello from client");

        send.write_frame(b"hello from server").await.unwrap();
        send.finish().await.unwrap();

        // Wait for client to signal it's done reading before dropping connection
        let _ = done_rx.await;
    });

    let client_endpoint = create_client().unwrap();
    let client_conn = connect(&client_endpoint, actual_addr).await.unwrap();
    let (mut send, mut recv) = client_conn.open_stream().await.unwrap();

    send.write_frame(b"hello from client").await.unwrap();
    send.finish().await.unwrap();

    let response = recv.read_frame(1024).await.unwrap();
    assert_eq!(&response[..], b"hello from server");

    // Signal server we're done
    let _ = done_tx.send(());
    timeout(Duration::from_secs(5), server_task).await.unwrap().unwrap();
}

#[tokio::test]
async fn multiple_streams() {
    let (key_der, cert_der) = gen_test_cert();

    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = create_server(bind_addr, &key_der, &cert_der).await.unwrap();
    let actual_addr = server.local_addr().unwrap();

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let qc = QuicConnection::new(conn);

        for _ in 0..3u8 {
            let (mut send, mut recv) = qc.accept_stream().await.unwrap();
            let frame = recv.read_frame(1024).await.unwrap();
            send.write_frame(&[frame[0] + 10]).await.unwrap();
            send.finish().await.unwrap();
        }

        let _ = done_rx.await;
    });

    let client_endpoint = create_client().unwrap();
    let client_conn = connect(&client_endpoint, actual_addr).await.unwrap();

    for i in 0..3u8 {
        let (mut send, mut recv) = client_conn.open_stream().await.unwrap();
        send.write_frame(&[i]).await.unwrap();
        send.finish().await.unwrap();
        let resp = recv.read_frame(1024).await.unwrap();
        assert_eq!(resp[0], i + 10);
    }

    let _ = done_tx.send(());
    timeout(Duration::from_secs(5), server_task).await.unwrap().unwrap();
}

#[tokio::test]
async fn large_frame_transfer() {
    let (key_der, cert_der) = gen_test_cert();

    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = create_server(bind_addr, &key_der, &cert_der).await.unwrap();
    let actual_addr = server.local_addr().unwrap();

    let data_size = 1024 * 1024;
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let qc = QuicConnection::new(conn);
        let (mut send, mut recv) = qc.accept_stream().await.unwrap();

        let frame = recv.read_frame(data_size + 1024).await.unwrap();
        assert_eq!(frame.len(), data_size);
        assert!(frame.iter().all(|&b| b == 0xAB));

        send.write_frame(b"ok").await.unwrap();
        send.finish().await.unwrap();

        let _ = done_rx.await;
    });

    let client_endpoint = create_client().unwrap();
    let client_conn = connect(&client_endpoint, actual_addr).await.unwrap();
    let (mut send, mut recv) = client_conn.open_stream().await.unwrap();

    let payload = vec![0xAB; data_size];
    send.write_frame(&payload).await.unwrap();
    send.finish().await.unwrap();

    let resp = recv.read_frame(1024).await.unwrap();
    assert_eq!(&resp[..], b"ok");

    let _ = done_tx.send(());
    timeout(Duration::from_secs(10), server_task).await.unwrap().unwrap();
}
