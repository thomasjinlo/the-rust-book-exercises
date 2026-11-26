mod common;

use common::{Client, Server, config, watchdog};

#[tokio::test]
async fn ping_returns_pong() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        c.send(b"PING\n").await.unwrap();
        assert_eq!(c.line().await, "PONG");
        drop(c); // serve returns only after every connection has ended
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
        drop(c);
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
        drop(c);
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
        drop(c);
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
        drop(c);
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
        drop(c);
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
        drop(c);
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
        drop(c);
        let stats = server.stop().await;
        assert_eq!(stats.commands, 2);
    })
    .await;
}

#[tokio::test]
async fn one_mebibyte_line_without_newline_is_refused() {
    watchdog(async {
        let server = Server::start(config(4, 1024)).await;
        let mut c = Client::connect(server.addr).await;
        let chunk = [b'x'; 64 * 1024];
        for _ in 0..16 {
            // The server closes after max_line + 2 bytes; later writes may fail.
            if c.send(&chunk).await.is_err() {
                break;
            }
        }
        assert_eq!(c.line().await, "ERR line too long");
        c.expect_eof().await;
        drop(c);
        let stats = server.stop().await;
        assert_eq!(stats.commands, 0);
    })
    .await;
}
