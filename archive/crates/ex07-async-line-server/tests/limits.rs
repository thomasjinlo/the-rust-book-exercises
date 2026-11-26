mod common;

use std::time::{Duration, Instant};

use common::{Client, Server, WAIT, config, watchdog};
use ex07_async_line_server::Stats;

#[tokio::test]
async fn third_client_is_busy_until_a_slot_frees() {
    watchdog(async {
        let server = Server::start(config(2, 1024)).await;
        let mut a = Client::connect(server.addr).await;
        let mut b = Client::connect(server.addr).await;
        // A reply proves each connection is admitted and holds a slot.
        for c in [&mut a, &mut b] {
            c.send(b"PING\n").await.unwrap();
            assert_eq!(c.line().await, "PONG");
        }

        // The client sends nothing: ERR busy must not wait for a request.
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.line().await, "ERR busy");
        c.expect_eof().await;

        drop(a);
        // The server notices the close asynchronously, so retry until the
        // slot is free. The retry loop is bounded by a 10 s deadline.
        let deadline = Instant::now() + WAIT;
        loop {
            let mut d = Client::connect(server.addr).await;
            let _ = d.send(b"PING\n").await;
            if d.try_line().await.as_deref() == Some("PONG") {
                break;
            }
            assert!(Instant::now() < deadline, "slot was never freed");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let stats = server.stop().await;
        assert!(stats.rejected_busy >= 1);
    })
    .await;
}

#[tokio::test]
async fn idle_connection_gets_an_error_and_is_closed() {
    watchdog(async {
        let mut cfg = config(4, 1024);
        cfg.idle_timeout = Duration::from_millis(200);
        let server = Server::start(cfg).await;
        let mut c = Client::connect(server.addr).await;
        let started = Instant::now();
        // 5 s is 25x the configured timeout.
        let line = c.try_line_within(Duration::from_secs(5)).await;
        assert_eq!(line.as_deref(), Some("ERR idle timeout"));
        assert!(started.elapsed() >= Duration::from_millis(200));
        c.expect_eof().await;
        let stats = server.stop().await;
        assert_eq!(stats.closed_idle, 1);
    })
    .await;
}

#[tokio::test]
async fn shutdown_drops_an_idle_connection_when_the_grace_ends() {
    watchdog(async {
        let mut cfg = config(4, 1024);
        cfg.shutdown_grace = Duration::from_millis(300);
        let mut server = Server::start(cfg).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");

        let started = Instant::now();
        server.signal_shutdown();
        let stats = server.join().await;
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(300),
            "returned after {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "returned after {elapsed:?}"
        );
        // The connection is closed without a farewell message.
        c.expect_eof().await;
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.closed_idle, 0);
    })
    .await;
}

#[tokio::test]
async fn connections_finish_their_work_during_the_grace_period() {
    watchdog(async {
        let mut cfg = config(4, 1024);
        cfg.shutdown_grace = Duration::from_secs(30);
        let mut server = Server::start(cfg).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");

        server.signal_shutdown();
        // Commands issued after the signal are still answered.
        c.send(b"ECHO late\n").await.unwrap();
        assert_eq!(c.line().await, "late");
        c.send(b"QUIT\n").await.unwrap();
        assert_eq!(c.line().await, "BYE");
        // Returns as soon as the last connection ends, not after 30 s.
        let started = Instant::now();
        server.join().await;
        assert!(started.elapsed() < Duration::from_secs(5));
    })
    .await;
}

#[tokio::test]
async fn shutdown_stops_accepting_at_once() {
    watchdog(async {
        let mut cfg = config(4, 1024);
        cfg.shutdown_grace = Duration::from_secs(30);
        let mut server = Server::start(cfg).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");

        server.signal_shutdown();
        // The listener closes shortly after the signal; poll until the port
        // refuses connections, bounded by a 10 s deadline.
        let deadline = Instant::now() + WAIT;
        while tokio::net::TcpStream::connect(server.addr).await.is_ok() {
            assert!(
                Instant::now() < deadline,
                "server still accepts after shutdown"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        c.send(b"QUIT\n").await.unwrap();
        assert_eq!(c.line().await, "BYE");
        server.join().await;
    })
    .await;
}

#[tokio::test]
async fn stats_for_a_scripted_scenario_are_exact() {
    watchdog(async {
        let mut cfg = config(2, 16);
        cfg.idle_timeout = Duration::from_secs(1);
        cfg.shutdown_grace = Duration::from_millis(200);
        let server = Server::start(cfg).await;

        // A: four commands, ends with QUIT. The close is server-initiated, so
        // the slot is already free when the client sees EOF (R14).
        let mut a = Client::connect(server.addr).await;
        a.send(b"PING\nECHO hi\nFOO\nQUIT\n").await.unwrap();
        for want in ["PONG", "hi", "ERR unknown command", "BYE"] {
            assert_eq!(a.line().await, want);
        }
        a.expect_eof().await;

        // B: 17 bytes, one over max_line. Not counted as a command.
        let mut b = Client::connect(server.addr).await;
        b.send(b"xxxxxxxxxxxxxxxxx\n").await.unwrap();
        assert_eq!(b.line().await, "ERR line too long");
        b.expect_eof().await;

        // C: says nothing.
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.line().await, "ERR idle timeout");
        c.expect_eof().await;

        // D and E fill both slots; F is rejected.
        let mut d = Client::connect(server.addr).await;
        let mut e = Client::connect(server.addr).await;
        for client in [&mut d, &mut e] {
            client.send(b"PING\n").await.unwrap();
            assert_eq!(client.line().await, "PONG");
        }
        let mut f = Client::connect(server.addr).await;
        assert_eq!(f.line().await, "ERR busy");
        f.expect_eof().await;

        // D and E are dropped when the grace ends.
        let stats = server.stop().await;
        assert_eq!(
            stats,
            Stats {
                accepted: 5,
                rejected_busy: 1,
                closed_idle: 1,
                closed_too_long: 1,
                commands: 6,
            }
        );
        d.expect_eof().await;
        e.expect_eof().await;
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_hundred_clients_fifty_echoes_each() {
    watchdog(async {
        let server = Server::start(config(256, 1024)).await;
        let addr = server.addr;
        let clients: Vec<_> = (0..200)
            .map(|i| {
                tokio::spawn(async move {
                    let mut c = Client::connect(addr).await;
                    for j in 0..50 {
                        c.send(format!("ECHO c{i}-{j}\n").as_bytes()).await.unwrap();
                        assert_eq!(c.line().await, format!("c{i}-{j}"));
                    }
                    c.send(b"QUIT\n").await.unwrap();
                    assert_eq!(c.line().await, "BYE");
                })
            })
            .collect();
        for client in clients {
            client.await.expect("client task panicked");
        }
        let stats = server.stop().await;
        assert_eq!(
            stats,
            Stats {
                accepted: 200,
                rejected_busy: 0,
                closed_idle: 0,
                closed_too_long: 0,
                commands: 200 * 51,
            }
        );
    })
    .await;
}
