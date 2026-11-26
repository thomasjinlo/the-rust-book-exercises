mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::*;
use ex08_tcp_reverse_proxy::{Proxy, ProxyConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Notify;

fn config(backends: Vec<SocketAddr>, threshold: u32, cooldown: Duration) -> ProxyConfig {
    ProxyConfig {
        backends,
        connect_timeout: Duration::from_secs(5),
        eject_after_failures: threshold,
        eject_cooldown: cooldown,
    }
}

/// One full echo through the proxy, then waits until the proxy counts it as ended.
async fn one(proxy: &Proxy, served_so_far: u64) -> Vec<u8> {
    let reply = roundtrip(proxy.local_addr(), b"ping").await;
    wait_finished(proxy, served_so_far + 1).await;
    reply
}

/// Two echo backends. Connection `n` starts at backend `n % 2`.
async fn pair() -> (TestBackend, TestBackend, Fake) {
    (spawn_echo().await, spawn_echo().await, Fake::new())
}

#[tokio::test]
async fn connect_timeout_counts_as_a_failure_and_fails_over() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        // A 60 s delay against a 200 ms timeout: the timeout always wins.
        fake.set(a0, Mode::Delay(Duration::from_secs(60)));
        let mut cfg = config(vec![a0, a1], u32::MAX, Duration::from_secs(60));
        cfg.connect_timeout = Duration::from_millis(200);
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        assert_eq!(one(&proxy, 0).await, b"ping");
        assert_eq!(fake.log(), vec![a0, a1]);
        assert_eq!(proxy.stats().backend_connect_failures, 1);
    })
    .await;
}

#[tokio::test]
async fn real_refused_port_is_skipped_then_ejected() {
    within(60, async {
        let live = spawn_echo().await;
        let dead = refused_addr().await;
        let proxy = start_proxy(config(vec![dead, live.addr], 3, Duration::from_secs(60))).await;

        for i in 0..10 {
            assert_eq!(one(&proxy, i).await, b"ping");
        }
        // Connections 0, 2, 4 reach the dead backend; the third failure ejects it.
        let s = proxy.stats();
        assert_eq!(s.backend_connect_failures, 3);
        assert_eq!(
            (s.accepted, s.completed, s.no_backend_available),
            (10, 10, 0)
        );
        assert_eq!(live.accepted(), 10);
    })
    .await;
}

#[tokio::test]
async fn ejected_backend_gets_no_attempts_during_the_cooldown() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        fake.set(a0, Mode::Refuse);
        let cfg = config(vec![a0, a1], 2, Duration::from_secs(60));
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        for i in 0..20 {
            assert_eq!(one(&proxy, i).await, b"ping");
        }
        // Connections 0 and 2 fail on backend 0 and eject it.
        assert_eq!(fake.attempts(a0), 2);
        assert_eq!(fake.attempts(a1), 20);
        assert_eq!(proxy.stats().backend_connect_failures, 2);
    })
    .await;
}

#[tokio::test]
async fn a_success_resets_the_consecutive_failure_counter() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        let cfg = config(vec![a0, a1], 2, Duration::from_secs(60));
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        // Even connections try backend 0 first.
        fake.set(a0, Mode::Refuse);
        one(&proxy, 0).await; // conn 0: backend 0 fails (1 of 2)
        fake.set(a0, Mode::Pass);
        one(&proxy, 1).await; // conn 1: backend 1
        one(&proxy, 2).await; // conn 2: backend 0 succeeds, counter resets
        fake.set(a0, Mode::Refuse);
        one(&proxy, 3).await; // conn 3: backend 1
        one(&proxy, 4).await; // conn 4: backend 0 fails (1 of 2 again)
        one(&proxy, 5).await; // conn 5: backend 1
        assert_eq!(
            fake.attempts(a0),
            3,
            "one failure after the reset must not eject"
        );
        one(&proxy, 6).await; // conn 6: backend 0 fails (2 of 2): ejected
        one(&proxy, 7).await;
        one(&proxy, 8).await; // conn 8: backend 0 is skipped
        assert_eq!(fake.attempts(a0), 4);
    })
    .await;
}

#[tokio::test]
async fn after_the_cooldown_one_trial_decides_and_failure_re_ejects() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        fake.set(a0, Mode::Refuse);
        // 1 s cooldown: the checks that must fall inside a cooldown take milliseconds.
        let cfg = config(vec![a0, a1], 1, Duration::from_secs(1));
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        one(&proxy, 0).await; // conn 0: fails once, ejected
        assert_eq!(fake.attempts(a0), 1);
        one(&proxy, 1).await;
        one(&proxy, 2).await; // conn 2 would start at backend 0: skipped
        assert_eq!(fake.attempts(a0), 1);

        tokio::time::sleep(Duration::from_millis(1300)).await;
        // conn 3 starts at backend 1. The trial happens on conn 4, and fails.
        one(&proxy, 3).await;
        assert_eq!(fake.attempts(a0), 1);
        one(&proxy, 4).await;
        assert_eq!(fake.attempts(a0), 2, "exactly one trial after the cooldown");
        // The failed trial re-ejects for a fresh cooldown.
        one(&proxy, 5).await;
        one(&proxy, 6).await; // conn 6 would start at backend 0
        one(&proxy, 7).await;
        one(&proxy, 8).await; // conn 8 would start at backend 0
        assert_eq!(fake.attempts(a0), 2);
    })
    .await;
}

#[tokio::test]
async fn a_successful_trial_reinstates_the_backend() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        fake.set(a0, Mode::Refuse);
        let cfg = config(vec![a0, a1], 1, Duration::from_secs(1));
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        one(&proxy, 0).await; // ejected
        fake.set(a0, Mode::Pass);
        tokio::time::sleep(Duration::from_millis(1300)).await;
        one(&proxy, 1).await; // conn 1 starts at backend 1
        one(&proxy, 2).await; // conn 2: the trial reaches backend 0 and succeeds
        assert_eq!(b0.accepted(), 1);
        for i in 3..9 {
            one(&proxy, i).await;
        }
        // Reinstated: connections 4, 6, 8 start at backend 0 again.
        assert_eq!(b0.accepted(), 4);
        assert_eq!(proxy.stats().backend_connect_failures, 1);
    })
    .await;
}

#[tokio::test]
async fn only_one_trial_is_in_flight_at_a_time() {
    within(60, async {
        let (b0, b1, fake) = pair().await;
        let (a0, a1) = (b0.addr, b1.addr);
        fake.set(a0, Mode::Refuse);
        let cfg = config(vec![a0, a1], 1, Duration::from_secs(1));
        let proxy = start_proxy_with(cfg, fake.clone()).await;
        let addr = proxy.local_addr();

        one(&proxy, 0).await; // ejected
        tokio::time::sleep(Duration::from_millis(1300)).await;
        let gate = Arc::new(Notify::new());
        fake.set(a0, Mode::GatedPass(gate.clone()));

        one(&proxy, 1).await; // conn 1 starts at backend 1
        // Conn 2 takes the trial and parks in connect.
        let mut trial = TcpStream::connect(addr).await.unwrap();
        wait_until(10, "the trial attempt", || fake.attempts(a0) == 2).await;
        one(&proxy, 2).await; // conn 3 starts at backend 1
        // Conn 4 would start at backend 0, but the trial is still in flight: it must skip.
        assert_eq!(roundtrip(addr, b"skip").await, b"skip");
        assert_eq!(
            fake.attempts(a0),
            2,
            "no second attempt while the trial is outstanding"
        );

        // The trial succeeds and its client is served by backend 0.
        gate.notify_one();
        trial.write_all(b"t").await.unwrap();
        let mut got = [0u8; 1];
        trial.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"t");
        assert_eq!(b0.accepted(), 1);
    })
    .await;
}
