use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::Extension;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout_at};

use crate::bounds::{MAX_HTTP_HEADER_BYTES, MAX_HTTP_HEADERS};
use crate::service::{ConnectionDeadline, Publisher, router};

pub async fn serve<F>(
    listener: TcpListener,
    publisher: Arc<Publisher>,
    shutdown: F,
) -> Result<(), ()>
where
    F: Future<Output = ()> + Send,
{
    tokio::pin!(shutdown);
    let mut connections = JoinSet::new();
    let mut failed = false;

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|_| ())?;
                let Ok(permit) = publisher.try_admit_connection() else {
                    drop(stream);
                    continue;
                };
                let accepted_deadline = Instant::now() + publisher.request_deadline();
                let publisher = publisher.clone();
                connections.spawn(async move {
                    let _permit = permit;
                    let mut stream = stream;
                    let prefix = match timeout_at(accepted_deadline, read_http1_head(&mut stream)).await {
                        Ok(Ok(prefix)) => prefix,
                        Ok(Err(())) | Err(_) => return,
                    };
                    let app = router(publisher).layer(Extension(ConnectionDeadline(accepted_deadline)));
                    let service = TowerToHyperService::new(app);
                    let mut builder = http1::Builder::new();
                    builder
                        .timer(TokioTimer::new())
                        .header_read_timeout(accepted_deadline.saturating_duration_since(Instant::now()))
                        .max_headers(MAX_HTTP_HEADERS)
                        .max_buf_size(MAX_HTTP_HEADER_BYTES)
                        .keep_alive(false);
                    let connection = builder.serve_connection(
                        TokioIo::new(PrefixedIo::new(prefix, stream)),
                        service,
                    );
                    let _ = timeout_at(accepted_deadline, connection).await;
                });
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                if completed.is_some_and(|result| result.is_err()) {
                    failed = true;
                    break;
                }
            }
        }
    }
    drop(listener);

    if failed {
        connections.abort_all();
    }
    let drain_deadline = Instant::now() + publisher.request_deadline();
    while !connections.is_empty() {
        match timeout_at(drain_deadline, connections.join_next()).await {
            Ok(Some(Ok(()))) => {}
            Ok(Some(Err(_))) => failed = true,
            Ok(None) => break,
            Err(_) => {
                failed = true;
                connections.abort_all();
                while connections.join_next().await.is_some() {}
                break;
            }
        }
    }
    if failed { Err(()) } else { Ok(()) }
}

async fn read_http1_head(stream: &mut tokio::net::TcpStream) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::with_capacity(MAX_HTTP_HEADER_BYTES);
    let mut scan_from = 0usize;
    loop {
        if let Some(relative) = bytes[scan_from..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            let header_end = scan_from + relative + 4;
            let mut headers = [httparse::EMPTY_HEADER; MAX_HTTP_HEADERS];
            let mut request = httparse::Request::new(&mut headers);
            if request.parse(&bytes[..header_end]) != Ok(httparse::Status::Complete(header_end)) {
                return Err(());
            }
            return Ok(bytes);
        }
        if bytes.len() == MAX_HTTP_HEADER_BYTES {
            return Err(());
        }
        scan_from = bytes.len().saturating_sub(3);
        let remaining = MAX_HTTP_HEADER_BYTES - bytes.len();
        let mut chunk = [0u8; 1024];
        let read_limit = remaining.min(chunk.len());
        let count = stream
            .read(&mut chunk[..read_limit])
            .await
            .map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

struct PrefixedIo {
    prefix: Cursor<Vec<u8>>,
    stream: tokio::net::TcpStream,
}

impl PrefixedIo {
    fn new(prefix: Vec<u8>, stream: tokio::net::TcpStream) -> Self {
        Self {
            prefix: Cursor::new(prefix),
            stream,
        }
    }
}

impl AsyncRead for PrefixedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let position = self.prefix.position() as usize;
        if position < self.prefix.get_ref().len() {
            let count = buffer
                .remaining()
                .min(self.prefix.get_ref().len() - position);
            buffer.put_slice(&self.prefix.get_ref()[position..position + count]);
            self.prefix.set_position((position + count) as u64);
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.stream).poll_read(context, buffer)
        }
    }
}

impl AsyncWrite for PrefixedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stream).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;
    use crate::jwks::StaticJwksProvider;
    use crate::receipt::TestSigner;
    use crate::store::DurableStore;
    use crate::test_support::{config, fixture, jwks, secure_tempdir};

    const REQUEST_KEY: &str = "9a9cb28d5d7d7a631b9b4304f5a5fbbb0d24c86d192b572774ef7aa21a29c88d";

    fn test_publisher(config: crate::ServiceConfig) -> Arc<Publisher> {
        let store = DurableStore::open(&config.ledger_path).unwrap();
        Publisher::for_test(
            config,
            Arc::new(StaticJwksProvider::new(jwks("fixture-key"))),
            store,
            Arc::new(TestSigner::new("test-publisher-v1")),
        )
    }

    async fn incomplete_connection(address: SocketAddr) -> TcpStream {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let _ = stream
            .write_all(b"POST /v1/receipts HTTP/1.1\r\nHost: localhost\r\n")
            .await;
        stream
    }

    async fn wait_for_slots(publisher: &Publisher, expected: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while publisher.available_request_slots() != expected {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }

    async fn read_until_closed(stream: &mut TcpStream) {
        let mut response = Vec::new();
        let result =
            tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
                .await
                .expect("connection exceeded its absolute deadline");
        let _ = result;
    }

    async fn legitimate_request(address: SocketAddr) -> Vec<u8> {
        let fixture = fixture();
        let mut request = format!(
            "POST /v1/receipts HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nAuthorization: Bearer {}\r\nIdempotency-Key: {REQUEST_KEY}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            fixture.token,
            fixture.request_body.len()
        )
        .into_bytes();
        request.extend_from_slice(&fixture.request_body);
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(&request).await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        response
    }

    async fn raw_request(address: SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let _ = stream.write_all(request).await;
        let mut response = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
            .await
            .unwrap();
        response
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn thirty_two_slow_headers_obey_four_connection_slots_and_one_second_deadline() {
        let temp = secure_tempdir();
        let mut config = config(temp.path().join("publisher.ledger"));
        config.max_inflight_requests = 4;
        config.request_deadline_milliseconds = 1_000;
        let publisher = test_publisher(config);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_publisher = publisher.clone();
        let server = tokio::spawn(async move {
            serve(listener, server_publisher, async {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let mut streams = Vec::new();
        for _ in 0..32 {
            streams.push(incomplete_connection(address).await);
        }
        wait_for_slots(&publisher, 0).await;
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        for stream in &mut streams {
            read_until_closed(stream).await;
        }
        wait_for_slots(&publisher, 4).await;

        let response = legitimate_request(address).await;
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(
            response
                .windows(20)
                .any(|bytes| bytes == b"publisher_receipt_ba")
        );
        wait_for_slots(&publisher, 4).await;

        shutdown_tx.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .unwrap()
                .unwrap()
                .is_ok()
        );
        assert!(publisher.shutdown().await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn slow_header_stress_releases_all_resources() {
        let temp = secure_tempdir();
        let mut config = config(temp.path().join("publisher.ledger"));
        config.max_inflight_requests = 4;
        config.request_deadline_milliseconds = 100;
        let publisher = test_publisher(config);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_publisher = publisher.clone();
        let server = tokio::spawn(async move {
            serve(listener, server_publisher, async {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let mut clients = JoinSet::new();
        for _ in 0..128 {
            clients.spawn(async move {
                let mut stream = incomplete_connection(address).await;
                read_until_closed(&mut stream).await;
            });
        }
        while let Some(result) = clients.join_next().await {
            result.unwrap();
        }
        wait_for_slots(&publisher, 4).await;

        shutdown_tx.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .unwrap()
                .unwrap()
                .is_ok()
        );
        assert!(publisher.shutdown().await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parser_header_count_and_byte_limits_reject_before_recovery_request() {
        let temp = secure_tempdir();
        let mut config = config(temp.path().join("publisher.ledger"));
        config.max_inflight_requests = 1;
        config.request_deadline_milliseconds = 500;
        let publisher = test_publisher(config);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_publisher = publisher.clone();
        let server = tokio::spawn(async move {
            serve(listener, server_publisher, async {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let mut too_many = b"POST /v1/receipts HTTP/1.1\r\n".to_vec();
        for index in 0..=MAX_HTTP_HEADERS {
            too_many.extend_from_slice(format!("X-{index}: x\r\n").as_bytes());
        }
        too_many.extend_from_slice(b"\r\n");
        let response = raw_request(address, &too_many).await;
        assert!(response.is_empty());
        wait_for_slots(&publisher, 1).await;

        let too_large = format!(
            "POST /v1/receipts HTTP/1.1\r\nX-Fill: {}\r\n\r\n",
            "x".repeat(MAX_HTTP_HEADER_BYTES)
        );
        let response = raw_request(address, too_large.as_bytes()).await;
        assert!(response.is_empty());
        wait_for_slots(&publisher, 1).await;

        assert!(
            legitimate_request(address)
                .await
                .starts_with(b"HTTP/1.1 200 OK\r\n")
        );
        wait_for_slots(&publisher, 1).await;
        shutdown_tx.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .unwrap()
                .unwrap()
                .is_ok()
        );
        assert!(publisher.shutdown().await);
    }
}
