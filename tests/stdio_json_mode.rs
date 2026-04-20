//! Integration tests for `jurnalis-cli stdio-json` mode.
//!
//! These tests exercise the stdio-json protocol by sending JSONL requests to
//! the binary's stdin and validating structured JSONL responses on stdout.
//!
//! NOTE: The `stdio-json` mode is tracked by issue #168 and may not be
//! implemented yet. These tests are written against the protocol spec and
//! will pass once #168 is merged. They are gated behind `#[ignore]` until
//! the feature lands.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn cli_binary_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe()
        .expect("failed to get current exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("jurnalis-cli");
    path
}

/// Spawn the CLI in stdio-json mode.
fn spawn_stdio_json() -> std::process::Child {
    Command::new(cli_binary_path())
        .arg("stdio-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn jurnalis-cli stdio-json")
}

/// Send a JSON request and read one JSON response line.
fn send_request(
    stdin: &mut impl Write,
    stdout: &mut BufReader<impl std::io::Read>,
    request: &serde_json::Value,
) -> serde_json::Value {
    let request_str = serde_json::to_string(request).expect("failed to serialize request");
    writeln!(stdin, "{}", request_str).expect("failed to write request");
    stdin.flush().expect("failed to flush");

    let mut response_line = String::new();
    stdout
        .read_line(&mut response_line)
        .expect("failed to read response");

    serde_json::from_str(response_line.trim())
        .unwrap_or_else(|e| panic!("Invalid JSON response: {} — raw: {:?}", e, response_line))
}

// --------------------------------------------------------------------------
// Session bootstrap tests
// --------------------------------------------------------------------------

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_start_new_returns_success() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let request = serde_json::json!({
        "id": "req-1",
        "op": "start_new",
        "params": {"seed": 42}
    });

    let response = send_request(&mut stdin, &mut stdout, &request);

    assert_eq!(response["id"], "req-1");
    assert_eq!(response["ok"], true);
    assert!(
        response["data"]["text"].is_array(),
        "data.text should be an array: {:?}",
        response
    );
    assert!(
        response["data"]["state"].is_string(),
        "data.state should be a string: {:?}",
        response
    );

    // State should be valid JSON
    let state_str = response["data"]["state"].as_str().unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(state_str).is_ok(),
        "state should be valid JSON"
    );

    // Text should contain character creation prompts
    let text_arr = response["data"]["text"].as_array().unwrap();
    assert!(!text_arr.is_empty(), "text should not be empty");

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_start_new_without_seed() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let request = serde_json::json!({
        "id": "req-noseed",
        "op": "start_new",
        "params": {}
    });

    let response = send_request(&mut stdin, &mut stdout, &request);

    assert_eq!(response["id"], "req-noseed");
    assert_eq!(response["ok"], true);
    assert!(response["data"]["text"].is_array());
    assert!(response["data"]["state"].is_string());

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_input_processes_player_command() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // First, start a new game
    let start_req = serde_json::json!({
        "id": "start-1",
        "op": "start_new",
        "params": {"seed": 42}
    });
    let start_resp = send_request(&mut stdin, &mut stdout, &start_req);
    assert_eq!(start_resp["ok"], true);

    let state = start_resp["data"]["state"].as_str().unwrap().to_string();

    // Send "1" to choose a race during character creation
    let input_req = serde_json::json!({
        "id": "input-1",
        "op": "input",
        "params": {
            "state": state,
            "text": "1"
        }
    });
    let input_resp = send_request(&mut stdin, &mut stdout, &input_req);

    assert_eq!(input_resp["id"], "input-1");
    assert_eq!(input_resp["ok"], true);
    assert!(
        input_resp["data"]["text"].is_array(),
        "data.text should be array: {:?}",
        input_resp
    );
    assert!(
        input_resp["data"]["state"].is_string(),
        "data.state should be string: {:?}",
        input_resp
    );

    // The state should have advanced (different from start state)
    let new_state = input_resp["data"]["state"].as_str().unwrap();
    assert_ne!(new_state, state, "State should change after valid input");

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_full_character_creation_flow() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Start new game
    let start_req = serde_json::json!({
        "id": "start",
        "op": "start_new",
        "params": {"seed": 42}
    });
    let start_resp = send_request(&mut stdin, &mut stdout, &start_req);
    assert_eq!(start_resp["ok"], true);
    let mut state = start_resp["data"]["state"].as_str().unwrap().to_string();

    // Drive through character creation
    let creation_inputs = ["1", "1", "1", "15 14 13 12 10 8", "1 2", "TestHero"];
    for (i, input) in creation_inputs.iter().enumerate() {
        let req = serde_json::json!({
            "id": format!("cc-{}", i),
            "op": "input",
            "params": {
                "state": state,
                "text": input
            }
        });
        let resp = send_request(&mut stdin, &mut stdout, &req);
        assert_eq!(resp["ok"], true, "Step {} failed: {:?}", i, resp);
        state = resp["data"]["state"].as_str().unwrap().to_string();
    }

    // After character creation, send "look" to verify exploration mode
    let look_req = serde_json::json!({
        "id": "look-1",
        "op": "input",
        "params": {
            "state": state,
            "text": "look"
        }
    });
    let look_resp = send_request(&mut stdin, &mut stdout, &look_req);
    assert_eq!(look_resp["ok"], true);

    let text = look_resp["data"]["text"].as_array().unwrap();
    // "look" in exploration mode should produce location description text
    assert!(
        !text.is_empty(),
        "look should produce output in exploration mode"
    );

    drop(stdin);
    child.wait().ok();
}

// --------------------------------------------------------------------------
// Save/Load operations
// --------------------------------------------------------------------------

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_save_and_load_round_trip() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "jurnalis_stdio_save_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let mut child = Command::new(cli_binary_path())
        .arg("stdio-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&tmp_dir)
        .spawn()
        .expect("failed to spawn");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Start a new game
    let start_req = serde_json::json!({
        "id": "s1",
        "op": "start_new",
        "params": {"seed": 42}
    });
    let start_resp = send_request(&mut stdin, &mut stdout, &start_req);
    let state = start_resp["data"]["state"].as_str().unwrap().to_string();

    // Save the state
    let save_req = serde_json::json!({
        "id": "save-1",
        "op": "save",
        "params": {
            "state": state,
            "name": "testslot"
        }
    });
    let save_resp = send_request(&mut stdin, &mut stdout, &save_req);
    assert_eq!(save_resp["id"], "save-1");
    assert_eq!(save_resp["ok"], true);

    // Load it back
    let load_req = serde_json::json!({
        "id": "load-1",
        "op": "load",
        "params": {"name": "testslot"}
    });
    let load_resp = send_request(&mut stdin, &mut stdout, &load_req);
    assert_eq!(load_resp["id"], "load-1");
    assert_eq!(load_resp["ok"], true);
    assert!(load_resp["data"]["state"].is_string());

    // Loaded state should match what we saved
    let loaded_state = load_resp["data"]["state"].as_str().unwrap();
    assert_eq!(loaded_state, state, "Loaded state should match saved state");

    drop(stdin);
    child.wait().ok();
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_start_from_save() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "jurnalis_stdio_fromsave_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    // Pre-create a save file with a valid state
    let state = jurnalis_engine::new_game(42, false).state_json;
    let saves_dir = tmp_dir.join("saves");
    std::fs::create_dir_all(&saves_dir).unwrap();
    std::fs::write(saves_dir.join("mysave.json"), &state).unwrap();

    let mut child = Command::new(cli_binary_path())
        .arg("stdio-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&tmp_dir)
        .spawn()
        .expect("failed to spawn");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let req = serde_json::json!({
        "id": "from-save-1",
        "op": "start_from_save",
        "params": {"name": "mysave"}
    });
    let resp = send_request(&mut stdin, &mut stdout, &req);

    assert_eq!(resp["id"], "from-save-1");
    assert_eq!(resp["ok"], true);
    assert!(resp["data"]["text"].is_array());
    assert!(resp["data"]["state"].is_string());

    drop(stdin);
    child.wait().ok();
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// --------------------------------------------------------------------------
// Error handling
// --------------------------------------------------------------------------

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_unknown_operation_returns_error() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let req = serde_json::json!({
        "id": "bad-op",
        "op": "nonexistent_operation",
        "params": {}
    });
    let resp = send_request(&mut stdin, &mut stdout, &req);

    assert_eq!(resp["id"], "bad-op");
    assert_eq!(resp["ok"], false);
    assert!(
        resp["error"]["code"].is_string(),
        "Error should have a code field: {:?}",
        resp
    );
    assert!(
        resp["error"]["message"].is_string(),
        "Error should have a message field: {:?}",
        resp
    );

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_invalid_json_returns_error() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send malformed JSON
    writeln!(stdin, "{{not valid json").unwrap();
    stdin.flush().unwrap();

    let mut response_line = String::new();
    stdout.read_line(&mut response_line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(response_line.trim())
        .expect("Response to invalid JSON should itself be valid JSON");

    assert_eq!(resp["ok"], false);
    assert!(resp["error"]["code"].is_string());
    assert!(resp["error"]["message"].is_string());

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_load_nonexistent_returns_error() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "jurnalis_stdio_noload_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let mut child = Command::new(cli_binary_path())
        .arg("stdio-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&tmp_dir)
        .spawn()
        .expect("failed to spawn");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let req = serde_json::json!({
        "id": "load-missing",
        "op": "load",
        "params": {"name": "does_not_exist"}
    });
    let resp = send_request(&mut stdin, &mut stdout, &req);

    assert_eq!(resp["id"], "load-missing");
    assert_eq!(resp["ok"], false);
    assert!(resp["error"]["message"].is_string());

    drop(stdin);
    child.wait().ok();
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// --------------------------------------------------------------------------
// Protocol purity
// --------------------------------------------------------------------------

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_stdout_is_protocol_pure() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let child_stdout = child.stdout.take().unwrap();

    // Send a valid request
    let req = serde_json::json!({
        "id": "purity-1",
        "op": "start_new",
        "params": {"seed": 42}
    });
    let request_str = serde_json::to_string(&req).unwrap();
    writeln!(stdin, "{}", request_str).unwrap();
    stdin.flush().unwrap();

    // Send an invalid request
    writeln!(stdin, "not json at all").unwrap();
    stdin.flush().unwrap();

    // Send another valid request
    let req2 = serde_json::json!({
        "id": "purity-2",
        "op": "start_new",
        "params": {"seed": 99}
    });
    let request_str2 = serde_json::to_string(&req2).unwrap();
    writeln!(stdin, "{}", request_str2).unwrap();
    stdin.flush().unwrap();

    // Close stdin to signal EOF
    drop(stdin);

    // Read all stdout
    let reader = BufReader::new(child_stdout);
    let stdout_str: String = reader
        .lines()
        .map(|l| l.unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");

    // Every line on stdout must be valid JSON
    for (i, line) in stdout_str.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        assert!(
            serde_json::from_str::<serde_json::Value>(trimmed).is_ok(),
            "Line {} on stdout is not valid JSON: {:?}",
            i + 1,
            trimmed
        );
    }

    // Should have produced exactly 3 response lines (2 valid + 1 error for invalid JSON)
    let non_empty_lines: Vec<&str> = stdout_str.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        non_empty_lines.len(),
        3,
        "Expected exactly 3 responses (2 success + 1 error), got {}. Lines: {:?}",
        non_empty_lines.len(),
        non_empty_lines
    );

    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_eof_exits_cleanly() {
    let mut child = spawn_stdio_json();
    let stdin = child.stdin.take().unwrap();

    // Close stdin immediately
    drop(stdin);

    let output = child.wait_with_output().expect("failed to wait");
    assert!(
        output.status.success(),
        "stdio-json should exit cleanly on EOF, got: {:?}",
        output.status
    );

    // stdout should be empty (no requests = no responses)
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let non_empty: Vec<&str> = stdout_str.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        non_empty.is_empty(),
        "No requests means no output. Got: {:?}",
        non_empty
    );
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_multiple_requests_in_sequence() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send multiple start_new requests - each should get its own response
    for i in 0..3 {
        let req = serde_json::json!({
            "id": format!("seq-{}", i),
            "op": "start_new",
            "params": {"seed": 42 + i}
        });
        let resp = send_request(&mut stdin, &mut stdout, &req);
        assert_eq!(resp["id"], format!("seq-{}", i));
        assert_eq!(resp["ok"], true);
    }

    drop(stdin);
    child.wait().ok();
}

#[test]
#[ignore = "requires stdio-json mode from #168"]
fn stdio_json_request_ids_are_echoed_back() {
    let mut child = spawn_stdio_json();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let ids = ["alpha", "beta-123", "req_with_underscores", "42"];

    for id in ids {
        let req = serde_json::json!({
            "id": id,
            "op": "start_new",
            "params": {"seed": 42}
        });
        let resp = send_request(&mut stdin, &mut stdout, &req);
        assert_eq!(
            resp["id"].as_str().unwrap(),
            id,
            "Response ID should match request ID"
        );
    }

    drop(stdin);
    child.wait().ok();
}
