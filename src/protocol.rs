use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::Path;

use jurnalis_engine::{new_game, process_input, GameOutput};

// ============================================================================
// Request / Response envelope types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: Option<String>,
    pub op: Option<String>,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl Response {
    pub fn success(id: Option<String>, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<String>, code: &str, message: &str) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(ErrorPayload {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

// ============================================================================
// Session state held in memory during the protocol loop
// ============================================================================

struct Session {
    state_json: Option<String>,
    save_dir: std::path::PathBuf,
}

impl Session {
    fn new(save_dir: &Path) -> Self {
        Self {
            state_json: None,
            save_dir: save_dir.to_path_buf(),
        }
    }
}

// ============================================================================
// Protocol loop
// ============================================================================

/// Run the stdio-json protocol loop. Reads JSONL from `reader`, writes JSONL to `writer`.
/// Diagnostic messages go to `err_writer` (stderr in production).
pub fn run_protocol<R: BufRead, W: Write, E: Write>(
    reader: &mut R,
    writer: &mut W,
    _err_writer: &mut E,
    save_dir: &Path,
) -> io::Result<()> {
    let mut session = Session::new(save_dir);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // skip blank lines
        }

        let response = handle_line(trimmed, &mut session);
        let json = serde_json::to_string(&response).unwrap();
        writeln!(writer, "{}", json)?;
        writer.flush()?;
    }

    Ok(())
}

fn handle_line(line: &str, session: &mut Session) -> Response {
    // Try to parse the request envelope
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(_) => {
            return Response::error(None, "invalid_request", "invalid JSON");
        }
    };

    let id = req.id.clone();

    // Validate required fields
    let op = match &req.op {
        Some(op) => op.clone(),
        None => {
            return Response::error(id, "invalid_request", "missing required field: op");
        }
    };

    let params = req.params.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    dispatch_op(&op, id, &params, session)
}

fn dispatch_op(
    op: &str,
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    match op {
        "start_new" => op_start_new(id, params, session),
        "start_from_save" => op_start_from_save(id, params, session),
        "input" => op_input(id, params, session),
        "save" => op_save(id, params, session),
        "load" => op_load(id, params, session),
        "list_saves" => op_list_saves(id, session),
        #[cfg(feature = "dev")]
        "inject_state" => op_inject_state(id, params, session),
        _ => Response::error(id, "invalid_operation", &format!("unknown operation: {}", op)),
    }
}

// ============================================================================
// Operation handlers
// ============================================================================

fn game_output_to_result(output: &GameOutput) -> serde_json::Value {
    serde_json::json!({
        "text": output.text,
        "state_json": output.state_json,
        "state_changed": output.state_changed,
    })
}

fn op_start_new(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let seed = params.get("seed").and_then(|v| v.as_u64()).unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(42)
    });

    let output = new_game(seed, false);
    session.state_json = Some(output.state_json.clone());
    Response::success(id, game_output_to_result(&output))
}

fn op_start_from_save(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("autosave");
    let slot = sanitize_save_name(name);
    let path = session.save_dir.join(format!("{}.json", slot));

    let state_json = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Response::error(
                id,
                "load_error",
                &format!("failed to read save '{}': {}", slot, e),
            );
        }
    };

    // Validate through engine
    if let Err(msg) = jurnalis_engine::state::load_game(&state_json) {
        return Response::error(id, "load_error", &format!("invalid save data: {}", msg));
    }

    session.state_json = Some(state_json.clone());

    // Run a "look" to get current room description
    let output = process_input(&state_json, "look");
    session.state_json = Some(output.state_json.clone());

    let mut text = vec![format!(
        "Loaded game from {}.",
        path.file_name().unwrap().to_string_lossy()
    )];
    text.extend(output.text.iter().cloned());

    let result = serde_json::json!({
        "text": text,
        "state_json": output.state_json,
        "state_changed": false,
    });
    Response::success(id, result)
}

fn op_input(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let state_json = match &session.state_json {
        Some(s) => s.clone(),
        None => {
            return Response::error(
                id,
                "no_session",
                "no active session; call start_new or start_from_save first",
            );
        }
    };

    let text = match params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return Response::error(
                id,
                "invalid_request",
                "missing required field: params.text",
            );
        }
    };

    let output = process_input(&state_json, text);
    session.state_json = Some(output.state_json.clone());
    Response::success(id, game_output_to_result(&output))
}

fn op_save(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let state_json = match &session.state_json {
        Some(s) => s.clone(),
        None => {
            return Response::error(
                id,
                "no_session",
                "no active session; call start_new or start_from_save first",
            );
        }
    };

    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("autosave");
    let slot = sanitize_save_name(name);
    let path = session.save_dir.join(format!("{}.json", slot));

    if let Err(e) = std::fs::create_dir_all(&session.save_dir) {
        return Response::error(
            id,
            "save_error",
            &format!("failed to create save directory: {}", e),
        );
    }

    if let Err(e) = std::fs::write(&path, state_json.as_bytes()) {
        return Response::error(
            id,
            "save_error",
            &format!("failed to write save file: {}", e),
        );
    }

    let result = serde_json::json!({
        "saved_to": format!("{}.json", slot),
    });
    Response::success(id, result)
}

fn op_load(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("autosave");
    let slot = sanitize_save_name(name);
    let path = session.save_dir.join(format!("{}.json", slot));

    let state_json = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Response::error(
                id,
                "load_error",
                &format!("failed to read save '{}': {}", slot, e),
            );
        }
    };

    // Validate through engine
    if let Err(msg) = jurnalis_engine::state::load_game(&state_json) {
        return Response::error(id, "load_error", &format!("invalid save data: {}", msg));
    }

    session.state_json = Some(state_json.clone());

    // Run a "look" to get current room description
    let output = process_input(&state_json, "look");
    session.state_json = Some(output.state_json.clone());

    let mut text = vec![format!(
        "Loaded game from {}.",
        path.file_name().unwrap().to_string_lossy()
    )];
    text.extend(output.text.iter().cloned());

    let result = serde_json::json!({
        "text": text,
        "state_json": output.state_json,
        "state_changed": false,
    });
    Response::success(id, result)
}

fn op_list_saves(id: Option<String>, session: &Session) -> Response {
    let saves = match std::fs::read_dir(&session.save_dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".json") {
                        Some(name.trim_end_matches(".json").to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            names
        }
        Err(_) => Vec::new(), // No saves directory yet is not an error
    };

    let result = serde_json::json!({ "saves": saves });
    Response::success(id, result)
}

#[cfg(feature = "dev")]
fn op_inject_state(
    id: Option<String>,
    params: &serde_json::Value,
    session: &mut Session,
) -> Response {
    let state_value = match params.get("state") {
        Some(v) => v,
        None => {
            return Response::error(
                id,
                "invalid_request",
                "missing required field: params.state",
            );
        }
    };

    let state_json = serde_json::to_string(state_value).unwrap();

    // Validate through engine
    if let Err(msg) = jurnalis_engine::state::load_game(&state_json) {
        return Response::error(id, "invalid_state", &format!("invalid state: {}", msg));
    }

    session.state_json = Some(state_json.clone());

    let result = serde_json::json!({
        "text": ["[Dev] State injected."],
        "state_json": state_json,
        "state_changed": true,
    });
    Response::success(id, result)
}

// ============================================================================
// Helpers
// ============================================================================

fn sanitize_save_name(raw: &str) -> String {
    let name = raw.trim();
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        name.to_string()
    } else {
        "autosave".to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn run_protocol_with_input(input: &str) -> Vec<Response> {
        let tmp = std::env::temp_dir().join(format!(
            "jurnalis_cli_proto_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut reader = Cursor::new(input.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut err = Vec::new();

        run_protocol(&mut reader, &mut output, &mut err, &tmp).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let responses: Vec<Response> = output_str
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        std::fs::remove_dir_all(&tmp).ok();
        responses
    }

    fn run_protocol_with_save_dir(input: &str, save_dir: &Path) -> Vec<Response> {
        let mut reader = Cursor::new(input.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut err = Vec::new();

        run_protocol(&mut reader, &mut output, &mut err, save_dir).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        output_str
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn malformed_json_returns_error_with_null_id() {
        let responses = run_protocol_with_input("not json at all\n");
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        assert!(r.id.is_none());
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_request");
        assert!(err.message.contains("invalid JSON"));
    }

    #[test]
    fn missing_op_field_returns_error() {
        let input = r#"{"id":"bad"}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        assert_eq!(r.id.as_deref(), Some("bad"));
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_request");
        assert!(err.message.contains("op"));
    }

    #[test]
    fn unknown_operation_returns_error() {
        let input = r#"{"id":"1","op":"unknown_op"}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        assert_eq!(r.id.as_deref(), Some("1"));
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_operation");
    }

    #[test]
    fn start_new_returns_game_output() {
        let input = r#"{"id":"1","op":"start_new","params":{"seed":42}}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(r.ok);
        assert_eq!(r.id.as_deref(), Some("1"));
        let result = r.result.as_ref().unwrap();
        assert!(result.get("text").unwrap().is_array());
        assert!(result.get("state_json").unwrap().is_string());
        assert_eq!(result.get("state_changed").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn input_without_session_returns_no_session_error() {
        let input = r#"{"id":"1","op":"input","params":{"text":"hello"}}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "no_session");
    }

    #[test]
    fn input_after_start_new_forwards_to_engine() {
        let input = concat!(
            r#"{"id":"1","op":"start_new","params":{"seed":42}}"#, "\n",
            r#"{"id":"2","op":"input","params":{"text":"1"}}"#, "\n",
        );
        let responses = run_protocol_with_input(input);
        assert_eq!(responses.len(), 2);
        assert!(responses[0].ok);
        assert!(responses[1].ok);
        let result = responses[1].result.as_ref().unwrap();
        assert!(result.get("text").unwrap().is_array());
        assert!(result.get("state_json").unwrap().is_string());
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = std::env::temp_dir().join(format!(
            "jurnalis_cli_proto_saveload_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let input = concat!(
            r#"{"id":"1","op":"start_new","params":{"seed":42}}"#, "\n",
            r#"{"id":"2","op":"save","params":{"name":"test_slot"}}"#, "\n",
            r#"{"id":"3","op":"load","params":{"name":"test_slot"}}"#, "\n",
        );

        let responses = run_protocol_with_save_dir(input, &tmp);
        assert_eq!(responses.len(), 3);

        // start_new succeeds
        assert!(responses[0].ok);

        // save succeeds
        assert!(responses[1].ok);
        let save_result = responses[1].result.as_ref().unwrap();
        assert_eq!(
            save_result.get("saved_to").unwrap().as_str(),
            Some("test_slot.json")
        );

        // load succeeds
        assert!(responses[2].ok);
        let load_result = responses[2].result.as_ref().unwrap();
        let text = load_result.get("text").unwrap().as_array().unwrap();
        assert!(text.iter().any(|t| t.as_str().unwrap().contains("Loaded game from test_slot.json")));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn save_without_session_returns_no_session_error() {
        let input = r#"{"id":"1","op":"save"}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "no_session");
    }

    #[test]
    fn list_saves_returns_empty_when_no_saves() {
        let input = r#"{"id":"1","op":"list_saves"}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(r.ok);
        let result = r.result.as_ref().unwrap();
        let saves = result.get("saves").unwrap().as_array().unwrap();
        assert!(saves.is_empty());
    }

    #[test]
    fn list_saves_returns_saved_files() {
        let tmp = std::env::temp_dir().join(format!(
            "jurnalis_cli_proto_list_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let input = concat!(
            r#"{"id":"1","op":"start_new","params":{"seed":42}}"#, "\n",
            r#"{"id":"2","op":"save","params":{"name":"alpha"}}"#, "\n",
            r#"{"id":"3","op":"save","params":{"name":"beta"}}"#, "\n",
            r#"{"id":"4","op":"list_saves"}"#, "\n",
        );

        let responses = run_protocol_with_save_dir(input, &tmp);
        assert_eq!(responses.len(), 4);
        assert!(responses[3].ok);
        let result = responses[3].result.as_ref().unwrap();
        let saves: Vec<&str> = result
            .get("saves")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(saves.contains(&"alpha"));
        assert!(saves.contains(&"beta"));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn load_nonexistent_save_returns_load_error() {
        let input = r#"{"id":"1","op":"load","params":{"name":"nonexistent"}}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "load_error");
    }

    #[test]
    fn start_from_save_loads_and_returns_output() {
        let tmp = std::env::temp_dir().join(format!(
            "jurnalis_cli_proto_fromstate_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a valid save first using the engine directly
        let game_output = new_game(42, false);
        std::fs::write(tmp.join("mysave.json"), &game_output.state_json).unwrap();

        let input = r#"{"id":"1","op":"start_from_save","params":{"name":"mysave"}}"#.to_string() + "\n";
        let responses = run_protocol_with_save_dir(&input, &tmp);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(r.ok, "Expected success, got: {:?}", r.error);
        let result = r.result.as_ref().unwrap();
        let text = result.get("text").unwrap().as_array().unwrap();
        assert!(text.iter().any(|t| t.as_str().unwrap().contains("Loaded game from mysave.json")));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn blank_lines_are_skipped() {
        let input = "\n\n".to_string()
            + r#"{"id":"1","op":"start_new","params":{"seed":42}}"#
            + "\n\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        assert!(responses[0].ok);
    }

    #[test]
    fn input_missing_text_param_returns_error() {
        let input = concat!(
            r#"{"id":"1","op":"start_new","params":{"seed":42}}"#, "\n",
            r#"{"id":"2","op":"input","params":{}}"#, "\n",
        );
        let responses = run_protocol_with_input(input);
        assert_eq!(responses.len(), 2);
        assert!(responses[0].ok);
        assert!(!responses[1].ok);
        let err = responses[1].error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_request");
        assert!(err.message.contains("text"));
    }

    #[test]
    fn multiple_requests_processed_sequentially() {
        let input = concat!(
            r#"{"id":"1","op":"start_new","params":{"seed":42}}"#, "\n",
            r#"{"id":"2","op":"input","params":{"text":"1"}}"#, "\n",
            r#"{"id":"3","op":"input","params":{"text":"1"}}"#, "\n",
        );
        let responses = run_protocol_with_input(input);
        assert_eq!(responses.len(), 3);
        for (i, r) in responses.iter().enumerate() {
            assert!(r.ok, "Response {} failed: {:?}", i, r.error);
            assert_eq!(r.id.as_deref(), Some(&format!("{}", i + 1)[..]));
        }
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inject_state_replaces_session_state() {
        // Create a valid state via engine
        let game_output = new_game(42, false);
        let state_value: serde_json::Value = serde_json::from_str(&game_output.state_json).unwrap();

        let req = serde_json::json!({
            "id": "1",
            "op": "inject_state",
            "params": { "state": state_value }
        });
        let input = serde_json::to_string(&req).unwrap() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(r.ok, "Expected success, got: {:?}", r.error);
        let result = r.result.as_ref().unwrap();
        let text = result.get("text").unwrap().as_array().unwrap();
        assert!(text.iter().any(|t| t.as_str().unwrap().contains("[Dev] State injected.")));
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inject_state_with_invalid_state_returns_error() {
        let req = serde_json::json!({
            "id": "1",
            "op": "inject_state",
            "params": { "state": {"invalid": true} }
        });
        let input = serde_json::to_string(&req).unwrap() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_state");
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inject_state_missing_state_param_returns_error() {
        let input = r#"{"id":"1","op":"inject_state","params":{}}"#.to_string() + "\n";
        let responses = run_protocol_with_input(&input);
        assert_eq!(responses.len(), 1);
        let r = &responses[0];
        assert!(!r.ok);
        let err = r.error.as_ref().unwrap();
        assert_eq!(err.code, "invalid_request");
    }
}
