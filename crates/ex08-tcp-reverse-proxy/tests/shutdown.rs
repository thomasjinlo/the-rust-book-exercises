mod common;

use std::time::{Duration, Instant};

use common::*;
use ex08_tcp_reverse_proxy::ProxyConfig;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn config(backends: Vec<std::net::SocketAddr>) -> ProxyConfig {
    ProxyConfig {
        backends,
        connect_timeout: Duration::from_secs(120),
        eject_after_failures: 3,
        eject_cooldown: Duration::from_secs(60),
    }
}

#[tokio::test]
async fn shutdown_aborts_a_stuck_connection_after_the_two_second_grace() {
    within(60, async {
        let silent = spawn_silent().await;
        let proxy = start_proxy(config(vec![silent.addr])).await;
        let addr = proxy.local_addr();

        let mut client = TcpStream::connect(addr).await.unwrap();
        wait_until(10, "the backend connection", || silent.accepted() == 1).await;

        let started = Instant::now();
        proxy.shutdown().await;
        let took = started.elapsed();
        // At least the grace (minus scheduling slack), below the 6 s ceiling.
        assert!(
            took >= Duration::from_millis(1900),
            "returned after {took:?}, before the grace"
        );
        assert!(took < Duration::from_secs(6), "returned after {took:?}");

        // The client side of the aborted connection is closed.
        let mut buf = [0u8; 8];
        match client.read(&mut buf).await {
            Ok(0) | Err(_) => {}
            Ok(n) => panic!("read {n} unexpected bytes"),
        }
        // The listening socket is gone.
        assert!(TcpStream::connect(addr).await.is_err());
    })
    .await;
}

#[tokio::test]
async fn shutdown_returns_early_when_in_flight_connections_finish() {
    within(60, async {
        let echo = spawn_echo().await;
        let proxy = start_proxy(config(vec![echo.addr])).await;
        let addr = proxy.local_addr();

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"1").await.unwrap();
        let mut b = [0u8; 1];
        client.read_exact(&mut b).await.unwrap();

        let started = Instant::now();
        let stopping = tokio::spawn(proxy.shutdown());
        // Accepting stops while the existing connection stays up.
        wait_until(10, "the listener to close", || {
            std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err()
        })
        .await;
        client.write_all(b"2").await.unwrap();
        client.read_exact(&mut b).await.unwrap();
        assert_eq!(
            &b, b"2",
            "an in-flight connection keeps working during the grace"
        );
        drop(client);

        stopping.await.unwrap();
        let took = started.elapsed();
        assert!(
            took < Duration::from_millis(1800),
            "waited the full grace: {took:?}"
        );
    })
    .await;
}

#[tokio::test]
async fn shutdown_with_nothing_in_flight_is_prompt() {
    within(60, async {
        let echo = spawn_echo().await;
        let proxy = start_proxy(config(vec![echo.addr])).await;
        assert_eq!(roundtrip(proxy.local_addr(), b"x").await, b"x");
        wait_finished(&proxy, 1).await;
        let started = Instant::now();
        proxy.shutdown().await;
        assert!(started.elapsed() < Duration::from_secs(1));
    })
    .await;
}

#[tokio::test]
async fn shutdown_ends_connections_that_are_still_connecting() {
    within(60, async {
        let hang = refused_addr().await;
        let fake = Fake::new();
        fake.set(hang, Mode::Hang);
        let proxy = start_proxy_with(config(vec![hang]), fake.clone()).await;

        let _client = TcpStream::connect(proxy.local_addr()).await.unwrap();
        wait_until(10, "the connect attempt", || fake.attempts(hang) == 1).await;
        let started = Instant::now();
        proxy.shutdown().await;
        assert!(started.elapsed() < Duration::from_secs(6));
    })
    .await;
}
