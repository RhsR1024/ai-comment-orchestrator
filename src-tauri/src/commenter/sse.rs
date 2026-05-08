use serde_json::Value;
use std::io::{BufRead, BufReader, Read};

pub fn collect_sse_content(reader: impl Read) -> Result<String, String> {
    let mut content = String::new();

    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|error| error.to_string())?;
        let Some(payload) = line.strip_prefix("data: ") else {
            continue;
        };

        if payload == "[DONE]" {
            break;
        }

        let value: Value = serde_json::from_str(payload).map_err(|error| error.to_string())?;
        if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() {
            content.push_str(delta);
        }
    }

    Ok(content)
}
