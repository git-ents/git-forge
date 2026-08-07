use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use gix_forge::{EntityOps, Issue, Review, ReviewTarget};
use test_support::{commit_file, init_repo};

const BIN: &str = env!("CARGO_BIN_EXE_git-forge");

#[test]
fn ui_crawls_every_internal_route() {
    let repo_dir = tempfile::tempdir().unwrap();
    init_repo(repo_dir.path());
    commit_file(
        repo_dir.path(),
        "README.md",
        "# UI crawl\n\nA markdown preview.\n",
        "add readme",
    );
    commit_file(
        repo_dir.path(),
        "guide.adoc",
        "= UI crawl guide\n\nAn AsciiDoc preview.\n",
        "add guide",
    );
    commit_file(
        repo_dir.path(),
        "src/lib.rs",
        "pub fn ui_crawl() -> &'static str { \"rust\" }\n",
        "add rust source",
    );
    commit_file(
        repo_dir.path(),
        "notes/plain.txt",
        "plain text for the source fallback\n",
        "add plain text",
    );

    let repo = gix::open(repo_dir.path()).unwrap();
    Issue {
        id: "ui-issue-1".to_owned(),
        status: "open".to_owned(),
        title: "UI crawl issue".to_owned(),
        body: "searchable ui crawl issue".to_owned(),
        labels: vec![],
        assignees: vec![],
        reporters: vec![],
        edit: None,
    }
    .save_in_repo(&repo)
    .unwrap();
    Review {
        id: "ui-review-1".to_owned(),
        status: "open".to_owned(),
        body: "searchable ui crawl review".to_owned(),
        reviewers: vec![],
        requesters: vec![],
        target: ReviewTarget::Commit {
            oid: "deadbeef".to_owned(),
        },
        edit: None,
    }
    .save_in_repo(&repo)
    .unwrap();

    let server = Server::spawn(repo_dir.path());
    let address = server.wait_for_url();
    let mut crawler = Crawler::new(address);
    crawler.wait_until_ready();
    crawler.enqueue("GET", "/", Vec::new());
    for target in [
        "/tree",
        "/issues",
        "/issues/ui-issue-1",
        "/reviews",
        "/reviews/ui-review-1",
        "/members",
        "/query",
        "/search",
    ] {
        crawler.enqueue("GET", target, Vec::new());
    }
    crawler.enqueue("POST", "/query", b"predicate=issue".to_vec());
    crawler.enqueue("POST", "/search", b"keyword=ui%20crawl".to_vec());
    crawler.run();

    for target in [
        "/",
        "/tree",
        "/issues",
        "/issues/ui-issue-1",
        "/reviews",
        "/reviews/ui-review-1",
        "/members",
        "/query",
        "/search",
    ] {
        assert!(
            crawler.seen_get(target),
            "explicit GET route was not requested: {target}"
        );
    }
    assert!(crawler.seen_post("/query", b"predicate=issue"));
    assert!(crawler.seen_post("/search", b"keyword=ui%20crawl"));
    assert!(crawler.seen_path("/_topcoat/assets/"));
    assert!(crawler.seen_query("view=preview"));
    assert!(crawler.seen_query("view=source"));
}

struct Server {
    child: Child,
    output: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

impl Server {
    fn spawn(repo_dir: &std::path::Path) -> Self {
        let mut child = Command::new(BIN)
            .current_dir(repo_dir)
            .args(["ui", "--host", "127.0.0.1", "--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            output,
            reader: Some(reader),
        }
    }

    fn wait_for_url(&self) -> SocketAddr {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "git-forge UI did not print its URL");
            let line = match self
                .output
                .recv_timeout(remaining.min(Duration::from_millis(100)))
            {
                Ok(line) => line,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(error) => panic!("waiting for git-forge UI URL: {error}"),
            };
            if let Some(port) = line.strip_prefix("http://127.0.0.1:") {
                return format!("127.0.0.1:{port}").parse().unwrap_or_else(|error| {
                    panic!("malformed git-forge UI URL {line:?}: {error}")
                });
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

struct Crawler {
    client: HttpClient,
    queue: VecDeque<Request>,
    seen: HashSet<RequestKey>,
}

impl Crawler {
    fn new(address: SocketAddr) -> Self {
        Self {
            client: HttpClient { address },
            queue: VecDeque::new(),
            seen: HashSet::new(),
        }
    }

    fn wait_until_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let request = Request {
            method: "GET".to_owned(),
            target: "/".to_owned(),
            body: Vec::new(),
        };
        loop {
            match self.client.request(&request) {
                Ok(response) => {
                    assert!(
                        (200..300).contains(&response.status),
                        "GET / returned HTTP {} while waiting for the UI",
                        response.status
                    );
                    return;
                }
                Err(error) if retryable(&error) && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => panic!("waiting for UI server: {error}"),
            }
        }
    }

    fn enqueue(&mut self, method: &str, target: &str, body: Vec<u8>) {
        self.queue.push_back(Request {
            method: method.to_owned(),
            target: target.to_owned(),
            body,
        });
    }

    fn run(&mut self) {
        while let Some(request) = self.queue.pop_front() {
            let key = RequestKey {
                method: request.method.clone(),
                target: request.target.clone(),
                body: request.body.clone(),
            };
            if !self.seen.insert(key) {
                continue;
            }
            let response = self.client.request(&request).unwrap_or_else(|error| {
                panic!("{} {} failed: {error}", request.method, request.target)
            });
            assert!(
                (200..300).contains(&response.status),
                "{} {} returned HTTP {}",
                request.method,
                request.target,
                response.status
            );
            if response.is_html() {
                let body = response.body_text().unwrap_or_else(|error| {
                    panic!(
                        "{} {} has malformed HTML: {error}",
                        request.method, request.target
                    )
                });
                self.enqueue_links(body);
                self.enqueue_forms(body, &request.target);
            }
        }
    }

    fn enqueue_links(&mut self, html: &str) {
        for href in attribute_values(html, "href") {
            if let Some(target) = internal_target(&href, "/") {
                self.enqueue("GET", &target, Vec::new());
            }
        }
    }

    fn enqueue_forms(&mut self, html: &str, current: &str) {
        for tag in form_tags(html) {
            let action = attribute_value(tag, "action")
                .and_then(|value| internal_target(&value, current))
                .unwrap_or_else(|| current.to_owned());
            let method = attribute_value(tag, "method")
                .unwrap_or_else(|| "get".to_owned())
                .to_ascii_lowercase();
            match method.as_str() {
                "get" => self.enqueue("GET", &action, Vec::new()),
                "post" => self.enqueue("POST", &action, form_body(&action)),
                other => panic!("unsupported form method {other:?} for {action}"),
            }
        }
    }

    fn seen_get(&self, target: &str) -> bool {
        self.seen
            .iter()
            .any(|key| key.method == "GET" && key.target == target)
    }

    fn seen_post(&self, target: &str, body: &[u8]) -> bool {
        self.seen
            .iter()
            .any(|key| key.method == "POST" && key.target == target && key.body == body)
    }

    fn seen_path(&self, path: &str) -> bool {
        self.seen.iter().any(|key| key.target.starts_with(path))
    }

    fn seen_query(&self, query: &str) -> bool {
        self.seen.iter().any(|key| key.target.contains(query))
    }
}

struct Request {
    method: String,
    target: String,
    body: Vec<u8>,
}

struct RequestKey {
    method: String,
    target: String,
    body: Vec<u8>,
}

impl PartialEq for RequestKey {
    fn eq(&self, other: &Self) -> bool {
        self.method == other.method && self.target == other.target && self.body == other.body
    }
}

impl Eq for RequestKey {}

impl std::hash::Hash for RequestKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.method.hash(state);
        self.target.hash(state);
        self.body.hash(state);
    }
}

struct HttpClient {
    address: SocketAddr,
}

impl HttpClient {
    fn request(&self, request: &Request) -> io::Result<Response> {
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let body = &request.body;
        write!(
            stream,
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            request.method, request.target, self.address
        )?;
        if request.method == "POST" {
            write!(
                stream,
                "Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n",
                body.len()
            )?;
        }
        stream.write_all(b"\r\n")?;
        if request.method == "POST" {
            stream.write_all(body)?;
        }
        stream.flush()?;

        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        Response::parse(&bytes)
    }
}

struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    fn parse(bytes: &[u8]) -> io::Result<Self> {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| invalid("response has no header terminator"))?;
        let header_text = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| invalid("response headers are not UTF-8"))?;
        let mut lines = header_text.split("\r\n");
        let status_line = lines
            .next()
            .ok_or_else(|| invalid("response has no status line"))?;
        let mut status_parts = status_line.split_whitespace();
        if status_parts.next() != Some("HTTP/1.1") {
            return Err(invalid("response has an invalid HTTP version"));
        }
        let status = status_parts
            .next()
            .ok_or_else(|| invalid("response has no status code"))?
            .parse::<u16>()
            .map_err(|_| invalid("response has an invalid status code"))?;
        let mut headers = HashMap::new();
        for line in lines {
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| invalid("response has a malformed header"))?;
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
        let raw_body = &bytes[header_end + 4..];
        let body = if headers
            .get("transfer-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
        {
            decode_chunked(raw_body)?
        } else if let Some(length) = headers.get("content-length") {
            let length = length
                .parse::<usize>()
                .map_err(|_| invalid("response has an invalid content length"))?;
            if raw_body.len() != length {
                return Err(invalid("response body does not match content length"));
            }
            raw_body.to_vec()
        } else {
            raw_body.to_vec()
        };
        Ok(Self {
            status,
            headers,
            body,
        })
    }

    fn is_html(&self) -> bool {
        self.headers
            .get("content-type")
            .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
    }

    fn body_text(&self) -> io::Result<&str> {
        std::str::from_utf8(&self.body).map_err(|_| invalid("HTML body is not UTF-8"))
    }
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::TimedOut
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::UnexpectedEof
    )
}

fn decode_chunked(mut bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = bytes
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| invalid("chunk has no size terminator"))?;
        let size_text = std::str::from_utf8(&bytes[..line_end])
            .map_err(|_| invalid("chunk size is not UTF-8"))?
            .split(';')
            .next()
            .unwrap_or_default()
            .trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| invalid("chunk has an invalid size"))?;
        bytes = &bytes[line_end + 2..];
        if size == 0 {
            if !bytes.starts_with(b"\r\n") {
                return Err(invalid("chunk trailers are malformed"));
            }
            return Ok(body);
        }
        if bytes.len() < size + 2 || &bytes[size..size + 2] != b"\r\n" {
            return Err(invalid("chunk body is truncated"));
        }
        body.extend_from_slice(&bytes[..size]);
        bytes = &bytes[size + 2..];
    }
}

fn form_tags(html: &str) -> Vec<&str> {
    let mut tags = Vec::new();
    let mut offset = 0;
    while let Some(relative) = html[offset..].find("<form") {
        let start = offset + relative;
        let end = html[start..]
            .find('>')
            .map(|relative_end| start + relative_end + 1)
            .unwrap_or_else(|| panic!("form tag is not terminated"));
        tags.push(&html[start..end]);
        offset = end;
    }
    tags
}

fn attribute_values(html: &str, name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut offset = 0;
    while let Some(relative) = html[offset..].find('<') {
        let start = offset + relative;
        let end = html[start..]
            .find('>')
            .map(|relative_end| start + relative_end + 1)
            .unwrap_or_else(|| panic!("HTML tag is not terminated"));
        if let Some(value) = attribute_value(&html[start..end], name) {
            values.push(value);
        }
        offset = end;
    }
    values
}

fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(relative) = tag[offset..].find(name) {
        let start = offset + relative;
        let before = start
            .checked_sub(1)
            .and_then(|index| tag.as_bytes().get(index));
        let after = tag.as_bytes().get(start + name.len());
        if before.is_none_or(|byte| !is_attribute_char(*byte))
            && after.is_none_or(|byte| !is_attribute_char(*byte))
        {
            let mut value_start = start + name.len();
            while tag
                .as_bytes()
                .get(value_start)
                .is_some_and(u8::is_ascii_whitespace)
            {
                value_start += 1;
            }
            if tag.as_bytes().get(value_start) == Some(&b'=') {
                value_start += 1;
                while tag
                    .as_bytes()
                    .get(value_start)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    value_start += 1;
                }
                let quote = *tag.as_bytes().get(value_start)?;
                if quote == b'"' || quote == b'\'' {
                    let value_start = value_start + 1;
                    let value_end = tag[value_start..].find(quote as char)? + value_start;
                    return Some(decode_entities(&tag[value_start..value_end]));
                }
            }
        }
        offset = start + name.len();
    }
    None
}

fn is_attribute_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#x27;", "'")
        .replace("&#34;", "\"")
}

fn internal_target(value: &str, current: &str) -> Option<String> {
    let value = decode_entities(value);
    let value = value.split('#').next().unwrap_or_default();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        return Some(value.to_owned());
    }
    if value.starts_with('?') {
        let path = current.split('?').next().unwrap_or(current);
        return Some(format!("{path}{value}"));
    }
    None
}

fn form_body(action: &str) -> Vec<u8> {
    if action.starts_with("/search") {
        b"keyword=ui%20crawl".to_vec()
    } else if action.starts_with("/query") {
        b"predicate=issue".to_vec()
    } else {
        Vec::new()
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
