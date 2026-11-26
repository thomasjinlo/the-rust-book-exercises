mod common;

use common::{Client, Rng, Server, config, watchdog};

#[tokio::test]
async fn ping_returns_pong() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn echo_returns_text_after_first_space() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"ECHO hello world\n").await.unwrap();
        assert_eq!(c.line().await, "hello world");
        // Only the first space is part of the command.
        c.send(b"ECHO  two  \n").await.unwrap();
        assert_eq!(c.line().await, " two  ");
        // Empty text is valid.
        c.send(b"ECHO \n").await.unwrap();
        assert_eq!(c.line().await, "");
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn unknown_commands_keep_the_connection_open() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        for bad in ["ECHO", "ECHOX hi", "ping", "PING ", "NOPE", ""] {
            c.send(format!("{bad}\n").as_bytes()).await.unwrap();
            assert_eq!(c.line().await, "ERR unknown command", "input {bad:?}");
        }
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn quit_says_bye_and_closes() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"QUIT\n").await.unwrap();
        assert_eq!(c.line().await, "BYE");
        c.expect_eof().await;
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn crlf_is_stripped_once() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\r\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        c.send(b"ECHO abc\r\n").await.unwrap();
        assert_eq!(c.line().await, "abc");
        // Only one trailing \r is removed; the second is echoed back.
        c.send(b"ECHO abc\r\r\n").await.unwrap();
        assert_eq!(c.line().await, "abc\r");
        // A bare \r inside the line is data.
        c.send(b"PI\rNG\n").await.unwrap();
        assert_eq!(c.line().await, "ERR unknown command");
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn one_byte_at_a_time() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        for b in b"ECHO split\r\nPING\n" {
            c.send(&[*b]).await.unwrap();
        }
        assert_eq!(c.line().await, "split");
        assert_eq!(c.line().await, "PONG");
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn thousand_pipelined_requests_answer_in_order() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        let mut batch = Vec::new();
        for i in 0..1000 {
            if i % 2 == 0 {
                batch.extend_from_slice(b"PING\n");
            } else {
                batch.extend_from_slice(format!("ECHO {i}\n").as_bytes());
            }
        }
        // One write: the server must keep unread requests buffered and answer all.
        c.send(&batch).await.unwrap();
        for i in 0..1000 {
            let expected = if i % 2 == 0 {
                "PONG".to_string()
            } else {
                i.to_string()
            };
            assert_eq!(c.line().await, expected, "reply {i}");
        }
        let stats = server.stop().await;
        assert_eq!(stats.commands, 1000);
    })
    .await;
}

#[tokio::test]
async fn line_at_the_limit_is_accepted_and_one_over_is_not() {
    watchdog(async {
        // "ECHO " + 11 bytes = 16 bytes, exactly max_line.
        let server = Server::start(config(8, 16)).await;
        let at_limit = format!("ECHO {}", "a".repeat(11));
        assert_eq!(at_limit.len(), 16);

        let mut c = Client::connect(server.addr).await;
        c.send(format!("{at_limit}\n").as_bytes()).await.unwrap();
        assert_eq!(c.line().await, "a".repeat(11));
        // The \r does not count towards the limit.
        c.send(format!("{at_limit}\r\n").as_bytes()).await.unwrap();
        assert_eq!(c.line().await, "a".repeat(11));

        for over in [format!("{at_limit}a\n"), format!("{at_limit}a\r\n")] {
            let mut c = Client::connect(server.addr).await;
            c.send(over.as_bytes()).await.unwrap();
            assert_eq!(c.line().await, "ERR line too long", "input {over:?}");
            c.expect_eof().await;
        }
        let stats = server.stop().await;
        assert_eq!(stats.closed_too_long, 2);
        assert_eq!(stats.commands, 2);
    })
    .await;
}

#[tokio::test]
async fn oversized_line_is_cut_off_without_buffering_it() {
    watchdog(async {
        const MAX_LINE: usize = 10_000;
        const CHUNK: usize = 4096;
        // The client is ready to send 10 MiB with no newline.
        const CHUNKS_AVAILABLE: usize = 10 * 1024 * 1024 / CHUNK;
        let server = Server::start(config(4, MAX_LINE)).await;
        let mut c = Client::connect(server.addr).await;

        // A server must answer once max_line + 2 bytes have arrived: chunk 3.
        // After each chunk the client waits briefly for the answer. Each wait
        // returns at once if the answer is there, and 250 ms is far longer
        // than a loopback reply takes. A server that buffers until it sees a
        // newline never answers, so the client gives up after 40 chunks.
        let mut sent = 0;
        let mut reply = None;
        let chunk = [b'x'; CHUNK];
        for _ in 0..CHUNKS_AVAILABLE.min(40) {
            if c.send(&chunk).await.is_err() {
                // The server already closed; the reply may still be readable.
                reply = c.try_line().await;
                break;
            }
            sent += 1;
            reply = c
                .try_line_within(std::time::Duration::from_millis(250))
                .await;
            if reply.is_some() {
                break;
            }
        }
        assert_eq!(reply.as_deref(), Some("ERR line too long"));
        assert!(
            sent <= 6,
            "client sent {sent} chunks of {CHUNK} bytes before the answer (max_line = {MAX_LINE})"
        );
        let stats = server.stop().await;
        assert_eq!(stats.closed_too_long, 1);
    })
    .await;
}

#[tokio::test]
async fn random_sessions_with_random_splits_match_a_model() {
    watchdog(async {
        let server = Server::start(config(8, 64)).await;
        for seed in 1..=40u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut payload = Vec::new();
            let mut expected = Vec::new();
            for _ in 0..(1 + rng.below(30)) {
                let kind = rng.below(4);
                let len = rng.below(20);
                let text: String = (0..len)
                    .map(|_| (b'a' + rng.below(26) as u8) as char)
                    .collect();
                let (line, reply) = match kind {
                    0 => ("PING".to_string(), "PONG".to_string()),
                    1 => (format!("ECHO {text}"), text.clone()),
                    2 => (format!("X{text}"), "ERR unknown command".to_string()),
                    _ => ("ECHO".to_string(), "ERR unknown command".to_string()),
                };
                payload.extend_from_slice(line.as_bytes());
                payload.extend_from_slice(if rng.below(2) == 0 { b"\n" } else { b"\r\n" });
                expected.push(reply);
            }
            payload.extend_from_slice(b"QUIT\n");
            expected.push("BYE".into());

            let mut c = Client::connect(server.addr).await;
            let mut rest = &payload[..];
            while !rest.is_empty() {
                let n = (1 + rng.below(7) as usize).min(rest.len());
                c.send(&rest[..n]).await.unwrap();
                rest = &rest[n..];
            }
            for (i, want) in expected.iter().enumerate() {
                assert_eq!(&c.line().await, want, "seed {seed}, reply {i}");
            }
            c.expect_eof().await;
        }
        server.stop().await;
    })
    .await;
}
