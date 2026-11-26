mod common;

use std::time::{Duration, Instant};

use common::{Client, Server, WAIT, config, watchdog};
use ex08_async_line_server::Stats;

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
        drop(b);
        let stats = server.stop().await;
        assert!(stats.rejected_busy >= 1);
    })
    .await;
}

#[tokio::test]
async fn shutdown_waits_for_open_connections() {
    watchdog(async {
        let mut server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");

        server.signal_shutdown();
        // Commands issued after the signal are still answered. The reply also
        // proves that `serve` had time to react to the signal.
        c.send(b"ECHO late\n").await.unwrap();
        assert_eq!(c.line().await, "late");
        assert!(
            !server.is_finished(),
            "serve returned with a connection open"
        );

        c.send(b"QUIT\n").await.unwrap();
        assert_eq!(c.line().await, "BYE");
        let stats = server.join().await;
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.commands, 3);
    })
    .await;
}

#[tokio::test]
async fn shutdown_stops_accepting() {
    watchdog(async {
        let mut server = Server::start(config(4, 1024)).await;
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
        drop(c);
        server.join().await;
    })
    .await;
}

#[tokio::test]
async fn stats_for_a_scripted_scenario_are_exact() {
    watchdog(async {
        let mut server = Server::start(config(2, 16)).await;

        // A: four commands, ends with QUIT. The close is server-initiated, so
        // the slot is already free when the client sees EOF (R12).
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

        // C and D fill both slots; E is rejected.
        let mut c = Client::connect(server.addr).await;
        let mut d = Client::connect(server.addr).await;
        for client in [&mut c, &mut d] {
            client.send(b"PING\n").await.unwrap();
            assert_eq!(client.line().await, "PONG");
        }
        let mut e = Client::connect(server.addr).await;
        assert_eq!(e.line().await, "ERR busy");
        e.expect_eof().await;

        // `serve` returns only after C and D close.
        server.signal_shutdown();
        drop(c);
        drop(d);
        let stats = server.join().await;
        assert_eq!(
            stats,
            Stats {
                accepted: 4,
                rejected_busy: 1,
                commands: 6,
            }
        );
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
                commands: 200 * 51,
            }
        );
    })
    .await;
}
