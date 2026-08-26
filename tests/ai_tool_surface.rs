//! What the `ai_api` tool surface actually returns.
//!
//! `tests/agentic_contract.rs` covers the argv surface — `simon describe`, `get`,
//! `snapshot` — and `tests/ontology_conformance.rs` covers readings. Neither
//! covers `AiDataApi::call_tool`, which is the surface an LLM driving simon
//! through MCP touches, and nothing else did either. Three fabrications lived
//! there undisturbed as a result: `uptime_seconds: 0` marked "would need
//! platform-specific impl" next to a reader that had one, a macOS CPU arm
//! publishing load average as `usage_percent`, and `unwrap_or_default()` turning
//! unread strings into `""`.
//!
//! These tests assert about *shape*, not about particular hardware, so they say
//! the same thing on a developer's desktop and on a CI runner with no GPU.

use serde_json::Value;
use simonlib::ai_api::AiDataApi;

fn api() -> AiDataApi {
    AiDataApi::new().expect("the tool surface must be constructible")
}

/// Walk a JSON value, calling `f` on every string with its path.
fn walk_strings(path: &str, v: &Value, f: &mut impl FnMut(&str, &str)) {
    match v {
        Value::String(s) => f(path, s),
        Value::Object(m) => {
            for (k, val) in m {
                walk_strings(&format!("{path}.{k}"), val, f);
            }
        }
        Value::Array(a) => {
            for (i, val) in a.iter().enumerate() {
                walk_strings(&format!("{path}[{i}]"), val, f);
            }
        }
        _ => {}
    }
}

/// Every tool that succeeds, with the data it returned.
fn successful_results() -> Vec<(String, Value)> {
    let mut api = api();
    let names: Vec<String> = api.list_tools().into_iter().map(|t| t.name).collect();
    assert!(
        !names.is_empty(),
        "the tool catalogue is empty, so nothing below tests anything"
    );

    names
        .into_iter()
        .filter_map(|name| {
            // A tool that needs parameters is allowed to refuse an empty call.
            // This is about what a *successful* result contains.
            let result = api.call_tool(&name, serde_json::json!({})).ok()?;
            let v = serde_json::to_value(&result).ok()?;
            if v.get("success").and_then(Value::as_bool) != Some(true) {
                return None;
            }
            Some((name, v.get("data").cloned().unwrap_or(Value::Null)))
        })
        .collect()
}

/// The absence-word guard, on the surface that lacked one.
///
/// `push_str_as` has rejected these strings in readings since the DMI "n/a"
/// finding, and `unknown_is_never_dressed_as_a_measurement` enforces it there.
/// The tool surface had no equivalent, and on the machine this was written on it
/// returned seventy of them: `get_usb_devices` reported `"speed": "Unknown"` for
/// thirty-odd devices and `get_bluetooth_devices` reported `"type": "Unknown"`
/// for nine, plus one empty `"address"`.
///
/// An agent cannot distinguish `"Unknown"` from a real value by looking at it —
/// that is the entire problem. `null` can be distinguished by anything.
#[test]
fn no_tool_returns_a_string_that_names_an_absence() {
    let mut bad: Vec<String> = Vec::new();

    for (name, data) in successful_results() {
        walk_strings(&name, &data, &mut |path, s| {
            if s.trim().is_empty() || simonlib::ontology::resolve::names_an_absence(s) {
                bad.push(format!("{path} = {s:?}"));
            }
        });
    }

    assert!(
        bad.is_empty(),
        "the tool surface returned {} value(s) naming an absence — these reach an \
         agent looking exactly like measurements. `call_tool` nulls these out at \
         the boundary, so a value arriving here means something bypassed it:\n{bad:#?}",
        bad.len()
    );
}

/// A tool that reports success has to have something to report.
///
/// `ToolResult::success` with `data: null` is a contradiction: the call did not
/// fail, so an agent will read the payload, and there is no payload. Either the
/// reading worked or it did not.
#[test]
fn a_successful_tool_call_carries_data() {
    let empty: Vec<String> = successful_results()
        .into_iter()
        .filter(|(_, data)| data.is_null())
        .map(|(name, _)| name)
        .collect();

    assert!(
        empty.is_empty(),
        "these tools reported success with no data: {empty:#?}"
    );
}

/// Every tool in the catalogue is reachable through `call_tool`.
///
/// The catalogue and the dispatch `match` are two hand-maintained lists of the
/// same names. A tool advertised but not wired answers "Unknown tool", which an
/// agent reads as its own mistake rather than as simon's.
#[test]
fn every_advertised_tool_is_dispatchable() {
    let mut api = api();
    let names: Vec<String> = api.list_tools().into_iter().map(|t| t.name).collect();

    let unreachable: Vec<String> = names
        .into_iter()
        .filter(|name| {
            let Ok(result) = api.call_tool(name, serde_json::json!({})) else {
                return false;
            };
            serde_json::to_value(&result)
                .ok()
                .and_then(|v| v.get("error").cloned())
                .and_then(|e| e.as_str().map(|s| s.contains("Unknown tool")))
                .unwrap_or(false)
        })
        .collect();

    assert!(
        unreachable.is_empty(),
        "advertised in the catalogue but not wired into `call_tool`: {unreachable:#?}"
    );
}
