use serde_json::Value;
use crate::events::AppEvent;

/// Tier 2 stateful accumulator for stream_event deltas.
/// Buffers partial text/tool-input between content_block_start and block_done.
#[derive(Debug, Default)]
pub struct DeltaAccumulator {
    active_block_type: Option<String>,
    text_buffer: String,
    tool_input_buffer: String,
}

impl DeltaAccumulator {
    pub fn new() -> Self { Self::default() }

    pub fn on_content_block_start(&mut self, block_type: &str) {
        self.active_block_type = Some(block_type.to_string());
        self.text_buffer.clear();
        self.tool_input_buffer.clear();
    }

    pub fn on_text_delta(&mut self, text: &str) {
        if self.active_block_type.as_deref() == Some("text") {
            self.text_buffer.push_str(text);
        }
    }

    pub fn on_input_json_delta(&mut self, json: &str) {
        if self.active_block_type.as_deref() == Some("tool_use") {
            self.tool_input_buffer.push_str(json);
        }
    }

    pub fn on_block_done(&mut self) {
        self.active_block_type = None;
        self.text_buffer.clear();
        self.tool_input_buffer.clear();
    }

    pub fn current_text(&self) -> Option<&str> {
        if self.active_block_type.as_deref() == Some("text") && !self.text_buffer.is_empty() {
            Some(&self.text_buffer)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn current_tool_input_json(&self) -> Option<&str> {
        if self.active_block_type.as_deref() == Some("tool_use") && !self.tool_input_buffer.is_empty() {
            Some(&self.tool_input_buffer)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Process a stream_event JSON line through the accumulator.
/// Returns Some(StreamDelta) when there's new text to show.
pub fn process_stream_event(v: &Value, run_id: usize, acc: &mut DeltaAccumulator) -> Option<AppEvent> {
    let event = v.get("event")?;
    let event_type = event.get("type")?.as_str()?;

    match event_type {
        "content_block_start" => {
            let block_type = event.get("content_block")?.get("type")?.as_str()?;
            acc.on_content_block_start(block_type);
            None
        }
        "content_block_delta" => {
            let delta = event.get("delta")?;
            let delta_type = delta.get("type")?.as_str()?;
            match delta_type {
                "text_delta" => {
                    let text = delta.get("text")?.as_str()?;
                    acc.on_text_delta(text);
                    Some(AppEvent::StreamDelta {
                        run_id,
                        text: acc.current_text()?.to_string(),
                    })
                }
                "input_json_delta" => {
                    let json = delta.get("partial_json")?.as_str()?;
                    acc.on_input_json_delta(json);
                    None
                }
                _ => None, // thinking_delta, signature_delta — skip
            }
        }
        "content_block_stop" => {
            acc.on_block_done();
            None
        }
        _ => None, // message_start, message_delta, message_stop — skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_text_deltas() {
        let mut acc = DeltaAccumulator::new();
        acc.on_content_block_start("text");
        acc.on_text_delta("Hello ");
        assert_eq!(acc.current_text(), Some("Hello "));
        acc.on_text_delta("world");
        assert_eq!(acc.current_text(), Some("Hello world"));
    }

    #[test]
    fn block_done_clears_buffer() {
        let mut acc = DeltaAccumulator::new();
        acc.on_content_block_start("text");
        acc.on_text_delta("Hello");
        acc.on_block_done();
        assert_eq!(acc.current_text(), None);
    }

    #[test]
    fn ignores_deltas_without_block_start() {
        let mut acc = DeltaAccumulator::new();
        acc.on_text_delta("orphan");
        assert_eq!(acc.current_text(), None);
    }

    #[test]
    fn tool_input_accumulation() {
        let mut acc = DeltaAccumulator::new();
        acc.on_content_block_start("tool_use");
        acc.on_input_json_delta(r#"{"file"#);
        acc.on_input_json_delta(r#"_path":"/foo"}"#);
        assert_eq!(acc.current_tool_input_json(), Some(r#"{"file_path":"/foo"}"#));
    }

    #[test]
    fn process_stream_event_text_delta() {
        let mut acc = DeltaAccumulator::new();
        // First: content_block_start
        let start = serde_json::json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}});
        let result = process_stream_event(&start, 0, &mut acc);
        assert!(result.is_none());

        // Then: text_delta
        let delta = serde_json::json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}});
        let result = process_stream_event(&delta, 0, &mut acc);
        assert!(matches!(result, Some(AppEvent::StreamDelta { text, .. }) if text == "Hello"));
    }

    #[test]
    fn process_stream_event_content_block_stop() {
        let mut acc = DeltaAccumulator::new();
        acc.on_content_block_start("text");
        acc.on_text_delta("Hello");

        let stop = serde_json::json!({"type":"stream_event","event":{"type":"content_block_stop","index":0}});
        process_stream_event(&stop, 0, &mut acc);
        assert_eq!(acc.current_text(), None);
    }
}
