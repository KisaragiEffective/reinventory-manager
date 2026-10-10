use anyhow::{bail, Result};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub trait ResponseExt {
    async fn deserialize_json<T: DeserializeOwned>(self) -> T;
}

impl ResponseExt for Response {
    async fn deserialize_json<T: DeserializeOwned>(self) -> T {
        let status = self.status();
        let result = match self.bytes().await {
            Ok(body) => decode_json_response(status, &body),
            Err(_) => Err(anyhow::anyhow!(
                "Failed to read response; HTTP status: {status}; response structure: <unavailable>"
            )),
        };
        match result {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
}

// Preserve field names and nesting, but never print response values or URLs.
pub fn redacted_json_structure(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(fields.iter()
            .map(|(key, value)| (key.clone(), redacted_json_structure(value)))
            .collect()),
        Value::Array(items) => Value::Array(items.iter()
            .map(redacted_json_structure).collect()),
        _ => Value::String("<redacted>".into()),
    }
}

fn decode_json_response<T: DeserializeOwned>(status: StatusCode, body: &[u8]) -> Result<T> {
    let value: Value = serde_json::from_slice(body).map_err(|_| anyhow::anyhow!(
        "Failed to decode response; HTTP status: {status}; response structure: <invalid JSON; body redacted>"
    ))?;
    if !status.is_success() {
        bail!("HTTP request failed; HTTP status: {status}; response structure: {}", redacted_json_structure(&value));
    }
    // Print only the field path: the underlying error may quote private values.
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    let decoded = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| anyhow::anyhow!(
        "Failed to decode response; HTTP status: {status}; JSON path: {}; response structure: {}",
        error.path(), redacted_json_structure(&value)
    ))?;
    deserializer.end().map_err(|_| anyhow::anyhow!(
        "Failed to decode response; HTTP status: {status}; response structure: <invalid JSON; body redacted>"
    ))?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct RecordVersion {
        global_version: u32,
    }

    #[test]
    fn missing_record_field_reports_redacted_array_and_status() {
        let body = br#"[{"userId":"U-private","token":"secret-token","email":"private@example.com","entity":{"localVersion":12345}}]"#;
        let error = decode_json_response::<Vec<RecordVersion>>(StatusCode::OK, body).unwrap_err().to_string();
        assert!(error.contains("200 OK"));
        assert!(error.contains("userId"));
        assert!(error.contains("localVersion"));
        for private in ["U-private", "secret-token", "private@example.com", "12345"] {
            assert!(!error.contains(private));
        }
    }

    #[test]
    fn invalid_field_values_are_not_included_in_errors() {
        let error = decode_json_response::<RecordVersion>(StatusCode::OK, br#"{"globalVersion":"private-value"}"#).unwrap_err().to_string();
        assert!(error.contains("JSON path: globalVersion"));
        assert!(!error.contains("private-value"));
    }

    #[test]
    fn failing_array_item_is_identified_without_quoting_its_value() {
        let error = decode_json_response::<Vec<RecordVersion>>(StatusCode::OK,
            br#"[{"globalVersion":7},{"globalVersion":"private-value"}]"#)
            .unwrap_err().to_string();
        assert!(error.contains("JSON path: [1].globalVersion"));
        assert!(!error.contains("private-value"));
    }

    #[test]
    fn http_errors_are_rejected_even_with_valid_json() {
        let error = decode_json_response::<RecordVersion>(StatusCode::FORBIDDEN, br#"{"globalVersion":12345}"#).unwrap_err().to_string();
        assert!(error.contains("403 Forbidden"));
        assert!(!error.contains("12345"));
    }

    #[test]
    fn non_json_response_is_redacted() {
        let error = decode_json_response::<Value>(StatusCode::BAD_GATEWAY, b"private-body").unwrap_err().to_string();
        assert!(error.contains("502 Bad Gateway"));
        assert!(!error.contains("private-body"));
    }

    #[tokio::test]
    async fn extension_deserializes_http_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).await.unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"globalVersion\":7}").await.unwrap();
        });
        let response = reqwest::Client::builder().no_proxy().build().unwrap()
            .get(format!("http://{address}")).send().await.unwrap();
        let record: RecordVersion = response.deserialize_json().await;
        assert_eq!(record, RecordVersion { global_version: 7 });
        server.await.unwrap();
    }
}
