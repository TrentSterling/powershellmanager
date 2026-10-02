use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

pub(crate) struct Fixture {
    endpoint: String,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Fixture {
    pub(crate) fn json(body: &str) -> Self {
        Self::reply(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ), Duration::ZERO, None)
    }

    fn reply(response: String, delay: Duration, cancel: Option<Arc<AtomicBool>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("own local HTTP fixture");
        let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let worker = std::thread::spawn(move || {
            let start = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(5),
                            "fixture was never requested"
                        );
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 16384);
            }
            let request = String::from_utf8(request).unwrap().to_lowercase();
            assert!(request.starts_with("get /latest http/1.1\r\n"));
            assert!(request.contains("user-agent: powershellmanager\r\n"));
            assert!(request.contains("accept: application/vnd.github+json\r\n"));
            if let Some(cancel) = cancel {
                cancel.store(true, Ordering::Release);
            }
            std::thread::sleep(delay);
            // Timeouts and oversize replies deliberately disconnect the client.
            let _ = stream.write_all(response.as_bytes());
        });
        Self {
            endpoint,
            worker: Some(worker),
        }
    }

    pub(crate) fn client(&self) -> ReleaseClient {
        ReleaseClient::new(self.endpoint.clone(), Duration::from_secs(2))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn stable_versions_use_semantic_precedence_and_reject_incomplete_or_invalid_tags() {
    for (latest, current, newer) in [
        ("0.4.10", "0.4.2", true),
        ("1.0.0", "0.99.99", true),
        ("0.4.2", "0.4.2", false),
        ("0.3.99", "0.4.2", false),
        ("0.4.2+build.2", "0.4.2+build.1", false),
        ("0.4.3+build.1", "0.4.2", true),
        ("0.4.2", "0.4.2-rc.1", true),
        ("9.0.0-rc.1", "0.4.2", false),
        ("9.invalid.0", "0.4.2", false),
        ("09.0.0", "0.4.2", false),
        ("9.0", "0.4.2", false),
        ("9.0.0.1", "0.4.2", false),
        ("v9.0.0", "0.4.2", false),
        ("9.0.0+", "0.4.2", false),
        ("9.0.0", "bad", false),
        ("", "0.4.2", false),
        (" 9.0.0 ", "0.4.2", false),
        ("18446744073709551616.0.0", "0.4.2", false),
    ] {
        assert_eq!(
            version_newer(latest, current),
            newer,
            "{latest} vs {current}"
        );
    }
    let client = ReleaseClient::default();
    assert_eq!(client.endpoint, RELEASE_API);
    assert_eq!(client.timeout, Duration::from_secs(10));
}

#[test]
fn release_metadata_accepts_stable_tags_and_keeps_the_download_destination_fixed() {
    for tag in ["v9.1.2", "9.1.2"] {
        let server = Fixture::json(&format!(
            r#"{{"tag_name":"{tag}","html_url":"https://untrusted.example/","extra":"ignored"}}"#
        ));
        let update = server.client().latest("0.4.2").unwrap().unwrap();
        assert_eq!(update.latest_version, "9.1.2");
        assert_eq!(update.download_url, RELEASE_PAGE);
    }
    for body in [
        r#"{"tag_name":"v9.0.0","draft":true}"#,
        r#"{"tag_name":"v9.0.0","prerelease":true}"#,
        r#"{"tag_name":"v9.0.0-rc.1"}"#,
        r#"{"tag_name":"v0.4.2"}"#,
        r#"{"tag_name":"v0.4.1"}"#,
        r#"{"tag_name":""}"#,
        r#"{"tag_name":"9.invalid.0"}"#,
    ] {
        let server = Fixture::json(body);
        assert_eq!(server.client().latest("0.4.2").unwrap(), None);
    }
}

#[test]
fn http_failures_bad_json_and_truncated_bodies_cannot_publish_an_update() {
    crate::action_tests::init_logging();
    let info = Arc::new(Mutex::new(None));
    for body in ["not json", "{}", r#"{"tag_name":99}"#] {
        let server = Fixture::json(body);
        spawn(
            server.client(),
            info.clone(),
            Arc::new(AtomicBool::new(false)),
            egui::Context::default(),
        )
        .unwrap()
        .join()
        .unwrap();
        assert_eq!(*info.lock().unwrap(), None);
    }
    for response in [
        "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{",
    ] {
        let server = Fixture::reply(response.into(), Duration::ZERO, None);
        assert!(check(
            &server.client(),
            &info,
            &AtomicBool::new(false),
            &egui::Context::default()
        )
        .is_err());
        assert_eq!(*info.lock().unwrap(), None);
    }
}

#[test]
fn response_limit_and_timeout_are_enforced_by_the_real_http_client() {
    let server = Fixture::json(&" ".repeat(MAX_RESPONSE_BYTES as usize + 1));
    assert_eq!(
        server.client().latest("0.4.2").unwrap_err(),
        "Release response exceeds 1 MiB"
    );
    drop(server);
    let server = Fixture::reply(
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        Duration::from_millis(100),
        None,
    );
    let client = ReleaseClient::new(server.endpoint.clone(), Duration::from_millis(20));
    assert!(client.latest("0.4.2").is_err());
}

#[test]
fn worker_publishes_and_repaints_only_for_a_live_newer_release() {
    let server = Fixture::json(r#"{"tag_name":"v9.0.0"}"#);
    let info = Arc::new(Mutex::new(None));
    let ctx = egui::Context::default();
    let repaints = Arc::new(AtomicUsize::new(0));
    let observed = repaints.clone();
    ctx.set_request_repaint_callback(move |_| {
        observed.fetch_add(1, Ordering::Relaxed);
    });
    spawn(
        server.client(),
        info.clone(),
        Arc::new(AtomicBool::new(false)),
        ctx.clone(),
    )
    .unwrap()
    .join()
    .unwrap();
    assert_eq!(
        info.lock().unwrap().as_ref().unwrap().latest_version,
        "9.0.0"
    );
    assert!(repaints.load(Ordering::Relaxed) > 0);
    let original = info.lock().unwrap().clone();
    let server = Fixture::json(r#"{"tag_name":"0.0.1"}"#);
    check(&server.client(), &info, &AtomicBool::new(false), &ctx).unwrap();
    assert_eq!(*info.lock().unwrap(), original);
}

#[test]
fn quitting_before_or_during_a_request_and_unavailable_state_are_safe() {
    let info = Mutex::new(None);
    let ctx = egui::Context::default();
    let client = ReleaseClient::new("not-a-url".into(), Duration::ZERO);
    check(&client, &info, &AtomicBool::new(true), &ctx).unwrap();
    let quitting = Arc::new(AtomicBool::new(false));
    let body = r#"{"tag_name":"v9.0.0"}"#;
    let server = Fixture::reply(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
        Duration::ZERO,
        Some(quitting.clone()),
    );
    check(&server.client(), &info, &quitting, &ctx).unwrap();
    assert_eq!(*info.lock().unwrap(), None);
    crate::action_tests::init_logging();
    let _ = std::panic::catch_unwind(|| {
        let _guard = info.lock().unwrap();
        panic!("owned test state poison");
    });
    let server = Fixture::json(r#"{"tag_name":"v9.0.0"}"#);
    assert_eq!(
        check(&server.client(), &info, &AtomicBool::new(false), &ctx).unwrap_err(),
        "Update state is unavailable"
    );
}
