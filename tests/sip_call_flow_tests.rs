//! Automated SIP call-flow tests.
//!
//! These tests exercise the real LCR SIP server end-to-end over UDP: they start
//! `LcrSipServer` in-process on an ephemeral port (backed by the seeded test
//! database and the real LCR routing engine), then act as a SIP UAC that places
//! actual calls and validates the full signaling flow.
//!
//! This is the automated replacement for driving the switch by hand with SIPp:
//! no external tools required, and it runs as part of `cargo test`.

use anyhow::{anyhow, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use redfire_switch::sip_call_server::{CallTiming, LcrSipServer};

/// Provision + seed the shared test database and return its URL.
async fn setup_test_db() -> Result<String> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://redfire:password@localhost/redfire_switch".to_string());

    redfire_switch::database::DatabaseService::provision_schema(&database_url).await?;
    redfire_switch::database::DatabaseService::seed_lcr_sample_data(&database_url).await?;

    Ok(database_url)
}

/// A minimal SIP UAC (caller) over UDP for driving test calls.
struct TestUac {
    socket: UdpSocket,
    server_addr: SocketAddr,
    local_addr: SocketAddr,
    call_id: String,
    from_tag: String,
    branch: String,
}

impl TestUac {
    async fn new(server_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let local_addr = socket.local_addr()?;
        let uniq = uuid::Uuid::new_v4().simple().to_string();
        Ok(Self {
            socket,
            server_addr,
            local_addr,
            call_id: format!("call-{uniq}@test"),
            from_tag: format!("ft-{}", &uniq[..8]),
            branch: format!("z9hG4bK{}", &uniq[..12]),
        })
    }

    fn invite(&self, ani: &str, dnis: &str) -> String {
        let sdp = format!(
            "v=0\r\n\
             o=uac 0 0 IN IP4 {ip}\r\n\
             s=Redfire Test Call\r\n\
             c=IN IP4 {ip}\r\n\
             t=0 0\r\n\
             m=audio 40000 RTP/AVP 0 8 101\r\n\
             a=rtpmap:0 PCMU/8000\r\n\
             a=rtpmap:8 PCMA/8000\r\n\
             a=rtpmap:101 telephone-event/8000\r\n\
             a=sendrecv\r\n",
            ip = self.local_addr.ip()
        );
        format!(
            "INVITE sip:{dnis}@{server} SIP/2.0\r\n\
             Via: SIP/2.0/UDP {local};branch={branch}\r\n\
             Max-Forwards: 70\r\n\
             From: \"Test Caller\" <sip:{ani}@{local}>;tag={from_tag}\r\n\
             To: <sip:{dnis}@{server}>\r\n\
             Call-ID: {call_id}\r\n\
             CSeq: 1 INVITE\r\n\
             Contact: <sip:{ani}@{local}>\r\n\
             User-Agent: Redfire-Test-UAC\r\n\
             Content-Type: application/sdp\r\n\
             Content-Length: {clen}\r\n\r\n\
             {sdp}",
            server = self.server_addr,
            local = self.local_addr,
            branch = self.branch,
            from_tag = self.from_tag,
            call_id = self.call_id,
            clen = sdp.len(),
        )
    }

    fn ack(&self, dnis: &str) -> String {
        format!(
            "ACK sip:{dnis}@{server} SIP/2.0\r\n\
             Via: SIP/2.0/UDP {local};branch={branch}\r\n\
             Max-Forwards: 70\r\n\
             From: <sip:caller@{local}>;tag={from_tag}\r\n\
             To: <sip:{dnis}@{server}>\r\n\
             Call-ID: {call_id}\r\n\
             CSeq: 1 ACK\r\n\
             Content-Length: 0\r\n\r\n",
            server = self.server_addr,
            local = self.local_addr,
            branch = self.branch,
            from_tag = self.from_tag,
            call_id = self.call_id,
        )
    }

    fn bye(&self, dnis: &str) -> String {
        format!(
            "BYE sip:{dnis}@{server} SIP/2.0\r\n\
             Via: SIP/2.0/UDP {local};branch={branch}2\r\n\
             Max-Forwards: 70\r\n\
             From: <sip:caller@{local}>;tag={from_tag}\r\n\
             To: <sip:{dnis}@{server}>\r\n\
             Call-ID: {call_id}\r\n\
             CSeq: 2 BYE\r\n\
             Content-Length: 0\r\n\r\n",
            server = self.server_addr,
            local = self.local_addr,
            branch = self.branch,
            from_tag = self.from_tag,
            call_id = self.call_id,
        )
    }

    async fn send(&self, msg: &str) -> Result<()> {
        self.socket.send_to(msg.as_bytes(), self.server_addr).await?;
        Ok(())
    }

    /// Wait for a SIP response with the given status code, tolerating and
    /// collecting any intervening provisional responses. Returns the collected
    /// status codes (in order) up to and including the matched one.
    async fn wait_for_status(&self, want: u16, overall: Duration) -> Result<Vec<u16>> {
        let mut seen = Vec::new();
        let deadline = tokio::time::Instant::now() + overall;
        let mut buf = vec![0u8; 4096];

        loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| anyhow!("timed out waiting for {want}; saw {seen:?}"))?;

            let (len, _from) = match timeout(remaining, self.socket.recv_from(&mut buf)).await {
                Ok(r) => r?,
                Err(_) => return Err(anyhow!("timed out waiting for {want}; saw {seen:?}")),
            };

            let msg = String::from_utf8_lossy(&buf[..len]);
            if let Some(code) = parse_status_code(&msg) {
                // Only track responses for our Call-ID.
                if msg.contains(&self.call_id) {
                    seen.push(code);
                    if code == want {
                        return Ok(seen);
                    }
                }
            }
        }
    }
}

/// Parse the numeric status code from a SIP response status line.
fn parse_status_code(msg: &str) -> Option<u16> {
    let first = msg.lines().next()?;
    if !first.starts_with("SIP/2.0") {
        return None;
    }
    first.split_whitespace().nth(1)?.parse().ok()
}

/// Start an LcrSipServer on an ephemeral port with fast (test) timing, and drive
/// its receive loop on a background task. Returns the server handle (to read its
/// address and to shut it down) plus the JoinHandle.
async fn start_server(database_url: &str) -> Result<(Arc<LcrSipServer>, tokio::task::JoinHandle<()>)> {
    let server = Arc::new(
        LcrSipServer::with_timing(
            "127.0.0.1:0".parse().unwrap(),
            database_url,
            CallTiming::fast(),
        )
        .await?,
    );
    let run_handle = {
        let server = server.clone();
        tokio::spawn(async move {
            let _ = server.run().await;
        })
    };
    Ok((server, run_handle))
}

#[tokio::test]
async fn test_basic_call_flow_answered() -> Result<()> {
    let database_url = setup_test_db().await?;
    let (server, run_handle) = start_server(&database_url).await?;
    let server_addr = server.local_addr();

    let uac = TestUac::new(server_addr).await?;

    // Place a call known to route in the seeded data: NYC -> SF.
    uac.send(&uac.invite("12125551234", "14155555678")).await?;

    // Expect 100 Trying, then 180 Ringing, then 200 OK.
    let seen = uac.wait_for_status(200, Duration::from_secs(5)).await?;
    assert!(seen.contains(&100), "should receive 100 Trying, saw {seen:?}");
    assert!(seen.contains(&180), "should receive 180 Ringing, saw {seen:?}");
    assert!(seen.contains(&200), "should receive 200 OK, saw {seen:?}");

    // Complete the handshake and tear down.
    uac.send(&uac.ack("14155555678")).await?;
    uac.send(&uac.bye("14155555678")).await?;

    // BYE must be answered with 200 OK.
    let bye_seen = uac.wait_for_status(200, Duration::from_secs(3)).await?;
    assert!(bye_seen.contains(&200), "BYE should be answered 200 OK");

    server.shutdown();
    let _ = timeout(Duration::from_secs(2), run_handle).await;
    Ok(())
}

#[tokio::test]
async fn test_call_to_unroutable_number_gets_404() -> Result<()> {
    let database_url = setup_test_db().await?;
    let (server, run_handle) = start_server(&database_url).await?;
    let server_addr = server.local_addr();

    let uac = TestUac::new(server_addr).await?;

    // A DNIS with no matching NANPA/route should fail LCR routing -> 404.
    uac.send(&uac.invite("12125551234", "99999999999")).await?;

    let seen = uac.wait_for_status(404, Duration::from_secs(5)).await?;
    assert!(seen.contains(&100), "should still get 100 Trying first");
    assert!(seen.contains(&404), "unroutable call should get 404, saw {seen:?}");

    server.shutdown();
    let _ = timeout(Duration::from_secs(2), run_handle).await;
    Ok(())
}

#[tokio::test]
async fn test_options_ping_answered() -> Result<()> {
    let database_url = setup_test_db().await?;
    let (server, run_handle) = start_server(&database_url).await?;
    let server_addr = server.local_addr();

    let uac = TestUac::new(server_addr).await?;
    let options = format!(
        "OPTIONS sip:ping@{server} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {local};branch={branch}\r\n\
         Max-Forwards: 70\r\n\
         From: <sip:ping@{local}>;tag={from_tag}\r\n\
         To: <sip:ping@{server}>\r\n\
         Call-ID: {call_id}\r\n\
         CSeq: 1 OPTIONS\r\n\
         Content-Length: 0\r\n\r\n",
        server = server_addr,
        local = uac.local_addr,
        branch = uac.branch,
        from_tag = uac.from_tag,
        call_id = uac.call_id,
    );
    uac.send(&options).await?;

    let seen = uac.wait_for_status(200, Duration::from_secs(3)).await?;
    assert!(seen.contains(&200), "OPTIONS ping should be answered 200 OK");

    server.shutdown();
    let _ = timeout(Duration::from_secs(2), run_handle).await;
    Ok(())
}

#[tokio::test]
async fn test_multiple_concurrent_calls() -> Result<()> {
    let database_url = setup_test_db().await?;
    let (server, run_handle) = start_server(&database_url).await?;
    let server_addr = server.local_addr();

    // Fire several independent calls concurrently; each should be answered.
    let mut handles = Vec::new();
    for _ in 0..5 {
        handles.push(tokio::spawn(async move {
            let uac = TestUac::new(server_addr).await?;
            uac.send(&uac.invite("12125551234", "14155555678")).await?;
            let seen = uac.wait_for_status(200, Duration::from_secs(6)).await?;
            uac.send(&uac.ack("14155555678")).await?;
            uac.send(&uac.bye("14155555678")).await?;
            let _ = uac.wait_for_status(200, Duration::from_secs(3)).await;
            Ok::<Vec<u16>, anyhow::Error>(seen)
        }));
    }

    let mut answered = 0;
    for h in handles {
        let seen = h.await??;
        if seen.contains(&200) {
            answered += 1;
        }
    }
    assert_eq!(answered, 5, "all concurrent calls should be answered");

    server.shutdown();
    let _ = timeout(Duration::from_secs(2), run_handle).await;
    Ok(())
}
