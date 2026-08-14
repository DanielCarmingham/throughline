//! Drives `tlflow mcp` over real stdio JSON-RPC.
//!
//! The unit tests in src/mcp.rs prove the tool functions. This proves the wire:
//! the handshake, the advertised identity, and the instructions — all of which
//! were wrong in the first working version despite every unit test passing.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

struct Server {
    child: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn start(dir: &std::path::Path) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tlflow"))
            .arg("mcp")
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn tlflow mcp");
        let out = BufReader::new(child.stdout.take().unwrap());
        Server { child, out }
    }

    fn send(&mut self, msg: serde_json::Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.out.read_line(&mut line).expect("read response");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad JSON {line:?}: {e}"))
    }

    fn handshake(&mut self) -> serde_json::Value {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}
            }
        }));
        let init = self.recv();
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        }));
        init
    }

    fn call(&mut self, id: i64, name: &str, args: serde_json::Value) -> serde_json::Value {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": name, "arguments": args}
        }));
        let res = self.recv();
        let r = &res["result"];
        // Prefer the structured payload; fall back to the text content block.
        if !r["structuredContent"].is_null() {
            return r["structuredContent"].clone();
        }
        let text = r["content"][0]["text"].as_str().unwrap_or("{}");
        serde_json::from_str(text).unwrap_or(serde_json::Value::Null)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n- [x] a  ^aaa\n      → shipped\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why\n- [ ] c  ^ccc\n      why\n",
    )
    .unwrap();
    d
}

#[test]
fn the_server_introduces_itself_as_throughline_not_as_the_sdk() {
    // rmcp's default handler reports its OWN crate name here, because
    // Implementation::from_build_env() expands env!("CARGO_CRATE_NAME") inside
    // rmcp. Every unit test passed while the server called itself "rmcp".
    let d = fixture();
    let mut s = Server::start(d.path());
    let init = s.handshake();
    assert_eq!(init["result"]["serverInfo"]["name"], "throughline");
    assert_eq!(
        init["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn the_handshake_carries_instructions_that_teach_the_vocabulary() {
    let d = fixture();
    let mut s = Server::start(d.path());
    let init = s.handshake();
    let instr = init["result"]["instructions"]
        .as_str()
        .expect("instructions must be present");
    for needle in ["behind Now", "result", "what does that change"] {
        assert!(
            instr.contains(needle),
            "instructions missing {needle:?}: {instr}"
        );
    }
}

#[test]
fn tools_are_advertised() {
    let d = fixture();
    let mut s = Server::start(d.path());
    s.handshake();
    s.send(serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let res = s.recv();
    let mut names: Vec<String> = res["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["add", "advance", "check", "line", "move_item", "now", "window"]
    );
}

#[test]
fn now_returns_the_item_ahead_and_check_reports_clean() {
    let d = fixture();
    let mut s = Server::start(d.path());
    s.handshake();

    let now = s.call(2, "now", serde_json::json!({}));
    assert_eq!(now["item"]["id"], "bbb");
    assert_eq!(now["item"]["behind_now"], false);

    let check = s.call(3, "check", serde_json::json!({}));
    assert_eq!(check["clean"], true);
}

#[test]
fn advance_writes_through_to_the_file_and_asks_what_it_changes() {
    let d = fixture();
    let mut s = Server::start(d.path());
    s.handshake();

    let w = s.call(
        2,
        "advance",
        serde_json::json!({"result": "it worked over the wire"}),
    );
    assert_eq!(w["ok"], true);
    assert!(w["what_does_that_change"]
        .as_str()
        .unwrap()
        .contains("what does that change"));

    // The file on disk must reflect it — this is the source of truth.
    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.contains("- [x] b  ^bbb"));
    assert!(text.contains("→ it worked over the wire"));
}

#[test]
fn add_without_placement_is_refused_over_the_wire() {
    let d = fixture();
    let mut s = Server::start(d.path());
    s.handshake();
    let w = s.call(2, "add", serde_json::json!({"title": "no placement"}));
    assert_eq!(w["ok"], false, "placement is required");
}
