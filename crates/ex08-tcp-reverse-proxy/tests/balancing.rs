mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::*;
use ex08_tcp_reverse_proxy::ProxyConfig;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;

/// Config with ejection effectively off, for tests about ordering only.
fn no_eject(backends: Vec<SocketAddr>) -> ProxyConfig {
    ProxyConfig {
        backends,
        connect_timeout: Duration::from_secs(5),
        eject_after_failures: u32::MAX,
        eject_cooldown: Duration::from_secs(60),
    }
}

#[tokio::test]
async fn three_hundred_connections_hit_each_backend_exactly_100_times() {
    within(60, async {
        let backends = [spawn_echo().await, spawn_echo().await, spawn_echo().await];
        let proxy = start_proxy(no_eject(backends.iter().map(|b| b.addr).collect())).await;
        let addr = proxy.local_addr();

        // Concurrent clients: the count per backend must not depend on accept order.
        let mut clients = JoinSet::new();
        for chunk in 0..6 {
            for i in 0..50 {
                clients.spawn(async move {
                    let msg = format!("hello {chunk}-{i}");
                    assert_eq!(roundtrip(addr, msg.as_bytes()).await, msg.as_bytes());
                });
            }
            while let Some(r) = clients.join_next().await {
                r.unwrap();
            }
        }
        wait_finished(&proxy, 300).await;

        for b in &backends {
            assert_eq!(b.accepted(), 100);
        }
        let s = proxy.stats();
        assert_eq!((s.accepted, s.completed), (300, 300));
        assert_eq!((s.backend_connect_failures, s.no_backend_available), (0, 0));
    })
    .await;
}

#[tokio::test]
async fn first_connection_goes_to_backend_zero_then_cycles_in_order() {
    within(60, async {
        let backends = [spawn_echo().await, spawn_echo().await, spawn_echo().await];
        let addrs: Vec<_> = backends.iter().map(|b| b.addr).collect();
        let fake = Fake::new();
        let proxy = start_proxy_with(no_eject(addrs.clone()), fake.clone()).await;

        for i in 0..7u64 {
            assert_eq!(roundtrip(proxy.local_addr(), b"x").await, b"x");
            wait_finished(&proxy, i + 1).await;
        }
        let expected: Vec<_> = [0, 1, 2, 0, 1, 2, 0].iter().map(|&i| addrs[i]).collect();
        assert_eq!(fake.log(), expected);
    })
    .await;
}

#[tokio::test]
async fn failover_tries_following_backends_in_order_each_at_most_once() {
    within(60, async {
        let live = spawn_echo().await;
        let (dead0, dead1) = (refused_addr().await, refused_addr().await);
        let addrs = vec![dead0, dead1, live.addr];
        let fake = Fake::new();
        fake.set(dead0, Mode::Refuse);
        fake.set(dead1, Mode::Refuse);
        let proxy = start_proxy_with(no_eject(addrs.clone()), fake.clone()).await;

        // Connection 0 starts at index 0, connection 1 at index 1.
        assert_eq!(roundtrip(proxy.local_addr(), b"a").await, b"a");
        assert_eq!(fake.log(), vec![dead0, dead1, live.addr]);
        assert_eq!(roundtrip(proxy.local_addr(), b"b").await, b"b");
        assert_eq!(fake.log()[3..], [dead1, live.addr]);
        // Connection 2 starts at the live backend and needs no failover.
        assert_eq!(roundtrip(proxy.local_addr(), b"c").await, b"c");
        assert_eq!(fake.log().len(), 6);

        wait_finished(&proxy, 3).await;
        let s = proxy.stats();
        assert_eq!(s.backend_connect_failures, 3);
        assert_eq!(s.no_backend_available, 0);
    })
    .await;
}

#[tokio::test]
async fn all_backends_down_closes_the_client_and_counts_it() {
    within(60, async {
        let addrs = vec![
            refused_addr().await,
            refused_addr().await,
            refused_addr().await,
        ];
        let fake = Fake::new();
        for a in &addrs {
            fake.set(*a, Mode::Refuse);
        }
        let proxy = start_proxy_with(no_eject(addrs.clone()), fake.clone()).await;

        assert_eq!(roundtrip(proxy.local_addr(), b"").await, b"");
        let s = proxy.stats();
        assert_eq!(s.no_backend_available, 1);
        assert_eq!(s.backend_connect_failures, 3);
        assert_eq!(
            (
                s.completed,
                s.bytes_client_to_backend,
                s.bytes_backend_to_client
            ),
            (0, 0, 0)
        );
        // Each backend exactly once, starting at index 0.
        assert_eq!(fake.log(), addrs);
    })
    .await;
}

#[tokio::test]
async fn zero_backends_closes_every_client() {
    within(60, async {
        let proxy = start_proxy(no_eject(vec![])).await;
        assert_eq!(roundtrip(proxy.local_addr(), b"").await, b"");
        let s = proxy.stats();
        assert_eq!(
            (
                s.accepted,
                s.no_backend_available,
                s.backend_connect_failures
            ),
            (1, 1, 0)
        );
    })
    .await;
}

/// Random up/down patterns. The expected attempt log comes from a model of the selection rule.
#[tokio::test]
async fn randomized_outage_patterns_match_the_round_robin_model() {
    within(120, async {
        let mut rng = Rng(0xE8);
        for round in 0..8 {
            let n = 2 + rng.below(4) as usize;
            let mut addrs = Vec::new();
            let mut up = Vec::new();
            for _ in 0..n {
                let is_up = rng.below(3) != 0;
                addrs.push(if is_up {
                    spawn_echo().await.addr
                } else {
                    refused_addr().await
                });
                up.push(is_up);
            }
            let fake = Fake::new();
            for (a, u) in addrs.iter().zip(&up) {
                if !u {
                    fake.set(*a, Mode::Refuse);
                }
            }
            let proxy = start_proxy_with(no_eject(addrs.clone()), fake.clone()).await;

            let mut expected = Vec::new();
            let mut expected_none = 0u64;
            let conns = 25u64;
            for c in 0..conns {
                let start = c as usize % n;
                let mut served = false;
                for k in 0..n {
                    let i = (start + k) % n;
                    expected.push(addrs[i]);
                    if up[i] {
                        served = true;
                        break;
                    }
                }
                if !served {
                    expected_none += 1;
                }
                let reply = roundtrip(proxy.local_addr(), b"ping").await;
                assert_eq!(
                    reply,
                    if served { &b"ping"[..] } else { &b""[..] },
                    "round {round}"
                );
                wait_finished(&proxy, c + 1).await;
            }
            assert_eq!(fake.log(), expected, "round {round}");
            assert_eq!(
                proxy.stats().no_backend_available,
                expected_none,
                "round {round}"
            );
        }
    })
    .await;
}

#[tokio::test]
async fn a_slow_connect_does_not_block_other_connections() {
    within(60, async {
        let (stuck, ok) = (spawn_echo().await, spawn_echo().await);
        let fake = Fake::new();
        fake.set(stuck.addr, Mode::Hang);
        let mut cfg = no_eject(vec![stuck.addr, ok.addr]);
        cfg.connect_timeout = Duration::from_secs(120);
        let proxy = start_proxy_with(cfg, fake.clone()).await;

        // Connection 0 hangs inside connect to backend 0 (120 s timeout, far beyond the test).
        let _hung = TcpStream::connect(proxy.local_addr()).await.unwrap();
        wait_until(10, "the hung attempt", || fake.attempts(stuck.addr) == 1).await;
        // Connection 1 starts at backend 1 and must be served meanwhile.
        assert_eq!(roundtrip(proxy.local_addr(), b"through").await, b"through");
        assert_eq!(ok.accepted(), 1);
    })
    .await;
}

#[tokio::test]
async fn local_addr_is_the_listener_address_and_stats_start_at_zero() {
    within(60, async {
        let listener = bind().await;
        let want = listener.local_addr().unwrap();
        let echo = spawn_echo().await;
        let proxy = ex08_tcp_reverse_proxy::Proxy::start(listener, no_eject(vec![echo.addr]));
        assert_eq!(proxy.local_addr(), want);
        assert_eq!(proxy.stats(), Default::default());
        let mut c = TcpStream::connect(want).await.unwrap();
        c.write_all(b"z").await.unwrap();
        let mut b = [0u8; 1];
        c.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, b"z");
    })
    .await;
}
