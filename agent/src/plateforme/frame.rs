//! What may be logged about a raw text frame from the platform: its `type`
//! tag, and its length — **never its content**.
//!
//! Platform frames carry credentials: `enrole` and `battement-recu` carry
//! the agent's bearer token, `ice-config` the TURN credential. A frame that
//! fails to parse (a platform one version ahead, one extra field) must
//! still be diagnosable without writing them to `agent.log`.

/// The frame's `type` tag, when it is a JSON object whose `type` is a
/// string; `None` otherwise (not JSON, not an object, no tag).
pub(crate) fn frame_type(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("type")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::frame_type;

    #[test]
    fn the_type_tag_is_extracted_and_nothing_else() {
        assert_eq!(
            frame_type(r#"{"type":"enrole","v":9,"jeton":"secret"}"#).as_deref(),
            Some("enrole")
        );
    }

    #[test]
    fn a_frame_without_a_string_tag_has_no_type() {
        assert_eq!(frame_type("not json"), None);
        assert_eq!(frame_type(r#"["enrole"]"#), None);
        assert_eq!(frame_type(r#"{"jeton":"secret"}"#), None);
        assert_eq!(frame_type(r#"{"type":7}"#), None);
    }
}
