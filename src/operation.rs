use std::sync::Arc;
use anyhow::{bail, Result};
use reqwest::header::AUTHORIZATION;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use reqwest::{Client, ClientBuilder};
use uuid::Uuid;
use crate::LoginInfo;
use crate::http_response::ResponseExt;
use crate::http_response::redacted_json_structure;
use crate::model::{AuthorizationInfo, DirectoryMetadata, AbsoluteInventoryPath, Record, RecordId, RecordType, UserId, UserLoginPostBody, UserLoginPostResponse};

use crate::cli::Platform;
static CLIENT: Lazy<Arc<Client>> = Lazy::new(|| {
    let c = ClientBuilder::new().user_agent("NeosVR-Inventory-Manager/0.1");

    #[cfg(feature = "https_os_native")]
    let c = c.use_native_tls();

    Arc::new(
        c
            .build()
            .expect("failed to initialize HTTP client")
    )
});

pub struct PreLogin;

impl PreLogin {
    pub async fn login(platform: Platform, login_info: Option<LoginInfo>) -> Result<LoggedIn> {
        let base_point = platform.base_url();
        if let Some(auth) = login_info {
            let mut req = CLIENT
                .post(format!("{base_point}/userSessions"));

            if let Some(x) = auth.get_totp() {
                req = req.header("TOTP", x.0.clone());
            }

            if platform == Platform::Resonite {
                req = req.header("UID", Uuid::new_v4().simple().to_string() + &Uuid::new_v4().simple().to_string());
            }

            let response = req
                .json(&UserLoginPostBody::create(auth, false).for_platform(platform))
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Login request failed; HTTP status unavailable"))?;
            let status = response.status();
            let body = response.bytes().await
                .map_err(|_| anyhow::anyhow!("Failed to read login response; HTTP status: {status}"))?;
            let token_res = parse_login_response(platform, status, &body)?;

            debug!("post 3");
            let using_token = token_res.to_authorization_info();
            let user_id = token_res.user_id;

            debug!("post 4");
            Ok(Self::from_session_data(platform, Some(user_id), Some(using_token)))
        } else {
            Ok(Self::from_session_data(platform, None, None))
        }
    }

    pub const fn from_session_data(platform: Platform, current_user: Option<UserId>, authorization_info: Option<AuthorizationInfo>) -> LoggedIn {
        LoggedIn {
            platform,
            authorization_info,
            current_user,
        }
    }
}

pub struct LoggedIn {
    platform: Platform,
    authorization_info: Option<AuthorizationInfo>,
    current_user: Option<UserId>,
}

impl LoggedIn {
    pub async fn logout(self) {
        let base_point = self.platform.base_url();
        if let Some(authorization_info) = self.authorization_info {
            let owner_id = authorization_info.owner_id.clone();
            CLIENT
                .delete(format!("{base_point}/userSessions/{owner_id}/{auth_token}", auth_token = authorization_info.token))
                .header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform))
                .send()
                .await
                .unwrap();
        }
    }

    pub async fn get_directory_items(&self, owner_id: UserId, path: AbsoluteInventoryPath) -> Vec<Record> {
        let base_point = self.platform.base_url();
        let authorization_info = &self.authorization_info;
        let path = path.to_uri_query_value();
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す


        let endpoint = format!("{base_point}/users/{owner_id}/records?path={path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = CLIENT.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform));
        }

        res
            .send()
            .await
            .unwrap()
            .deserialize_json()
            .await
    }

    pub async fn get_directory_metadata(&self, owner_id: UserId, path: AbsoluteInventoryPath) -> DirectoryMetadata {
        let base_point = self.platform.base_url();
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す
        let authorization_info = &self.authorization_info;
        let path = path.to_absolute_path();
        let endpoint = format!("{base_point}/users/{owner_id}/records/root/{path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = CLIENT.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform));
        }

        res
            .send()
            .await
            .unwrap()
            .deserialize_json()
            .await
    }

    pub async fn move_records(&self, owner_id: UserId, records_to_move: Vec<RecordId>, to: Vec<String>, keep_record_id: bool) {
        let base_point = self.platform.base_url();
        let authorization_info = &self.authorization_info;

        for record_id in records_to_move {
            debug!("checking {record_id}", record_id = &record_id);
            let find = self.get_record(owner_id.clone(), record_id.clone()).await;

            if let Some(found_record) = find {
                if found_record.record_type == RecordType::Directory {
                    // TODO: fix this
                    error!("Directories cannot be moved at this time. This is implement restriction. \
                Please see https://github.com/KisaragiEffective/neosvr-inventory-management/issues/36 for more info.");
                    return;
                }

                debug!("found, moving");

                let from = found_record.path.clone();

                // region delete old record
                {
                    let endpoint = format!("{base_point}/users/{owner_id}/records/{record_id}", owner_id = &owner_id);
                    let mut req = CLIENT.delete(endpoint);

                    if let Some(authorization_info) = authorization_info {
                        req = req.header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform));
                    }

                    let deleted = req
                        .send()
                        .await
                        .unwrap();

                    debug!("deleted: {deleted:?}");
                }
                // endregion
                // region insert
                {
                    debug!("insert!");
                    let record_id = if keep_record_id {
                        debug!("record id unchanged");
                        record_id
                    } else {
                        // GUIDは小文字が「推奨」されているため念の為小文字にしておく
                        let record_id = RecordId(format!("R-{}", Uuid::new_v4().to_string().to_lowercase()));
                        debug!("new record id: {record_id}", record_id = &record_id);
                        record_id
                    };

                    let endpoint = format!("{base_point}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);
                    debug!("endpoint: {endpoint}", endpoint = &endpoint);
                    let mut request = CLIENT.put(endpoint);

                    if let Some(authorization_info) = authorization_info {
                        debug!("auth set");
                        request = request.header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform));
                    }

                    let mut record = found_record.clone();
                    record.path = to.join("\\");
                    record.id = record_id.clone();

                    debug!("requesting...");
                    let res = request
                        .json(&record)
                        .send()
                        .await
                        .unwrap();
                    if res.status().is_success() {
                        info!("Success! {record_id} for {owner_id} was moved from {from} to {to}.", to = to.join("\\"), record_id = &record_id);
                    } else if res.status().is_client_error() {
                        error!("Client error ({status}): this is fatal bug. Please report this to bug tracker.", status = res.status());
                        // TODO: rollback
                    } else if res.status().is_server_error() {
                        error!("Server error ({status}): Please try again in later.", status = res.status());
                    } else {
                        warn!("Unhandled status code: {status}", status = res.status());
                    }
                    debug!("Response: {res:?}", res = &res);
                }
                // endregion
            } else {
                warn!("not found");
            }
        }
    }

    pub async fn get_record(&self, owner_id: UserId, record_id: RecordId) -> Option<Record> {
        let base_point = self.platform.base_url();
        let endpoint = format!("{base_point}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);

        let mut request = CLIENT
            .get(endpoint);

        if let Some(authorization_info) = &self.authorization_info {
            debug!("auth set");
            request = request.header(AUTHORIZATION, authorization_info.as_authorization_header_value(self.platform));
        }

        let res = request
            .send()
            .await
            .expect("HTTP connection error");

        match res.status().as_u16() {
            200 => {
                let record = res
                    .deserialize_json()
                    .await;

                Some(record)
            }
            403 => {
                error!("Unauthorized");
                None
            }
            404 => None,
            other_status => {
                warn!("Unhandled status code: {other_status}");
                None
            }
        }
    }
}


fn parse_login_response(platform: Platform, status: reqwest::StatusCode, body: &[u8]) -> Result<UserLoginPostResponse> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| anyhow::anyhow!("Login failed; HTTP status: {status}; response structure: <invalid JSON; body redacted>"))?;
    if !status.is_success() {
        bail!("Login failed; HTTP status: {status}; response structure: {}", redacted_json_structure(&value));
    }
    // Serde's error text can contain response values, so use a fixed diagnostic.
    let session = match platform {
        Platform::Neos => value.clone(),
        Platform::Resonite => value.get("entity").cloned().unwrap_or(serde_json::Value::Null),
    };
    serde_json::from_value(session).map_err(|_| anyhow::anyhow!(
        "Failed to decode login response; HTTP status: {status}; response structure: {}",
        redacted_json_structure(&value)
    ))
}

#[cfg(test)]
mod login_response_tests {
    use super::*;

    #[test]
    fn decode_failure_reports_status_and_nested_keys_without_values() {
        let body = br#"{"entity":{"userId":"U-private","token":"private-token","email":"private@example.com"},"items":[{"secret":"private-secret"}],"count":12345,"valid":true,"empty":null}"#;
        let error = parse_login_response(Platform::Neos, reqwest::StatusCode::OK, body).err().unwrap().to_string();
        assert!(error.contains("200 OK"));
        for key in ["entity", "userId", "token", "email", "items", "secret"] {
            assert!(error.contains(key));
        }
        for secret in ["U-private", "private-token", "private@example.com", "private-secret", "12345", "true", "null"] {
            assert!(!error.contains(secret));
        }
    }

    #[test]
    fn http_failure_is_not_accepted_as_a_session() {
        let body = br#"{"userId":"U-private","token":"private-token"}"#;
        let error = parse_login_response(Platform::Neos, reqwest::StatusCode::UNAUTHORIZED, body).err().unwrap().to_string();
        assert!(error.contains("401 Unauthorized"));
        assert!(!error.contains("U-private"));
        assert!(!error.contains("private-token"));
    }

    #[test]
    fn invalid_json_is_not_printed() {
        let error = parse_login_response(Platform::Neos, reqwest::StatusCode::BAD_GATEWAY, b"private-response").err().unwrap().to_string();
        assert!(error.contains("502 Bad Gateway"));
        assert!(error.contains("invalid JSON"));
        assert!(!error.contains("private-response"));
    }

    #[test]
    fn valid_session_still_decodes() {
        let session = parse_login_response(Platform::Neos, reqwest::StatusCode::OK, br#"{"userId":"U-test","token":"test-token"}"#).unwrap();
        assert_eq!(session.user_id.to_string(), "U-test");
        assert_eq!(session.token.to_string(), "test-token");
    }

    #[test]
    fn resonite_session_decodes_from_entity() {
        let body = br#"{"entity":{"userId":"U-test","token":"test-token","created":"2026-10-11T00:00:00Z","partitionKey":"ignored","rowKey":"ignored","rememberMe":false}}"#;
        let session = parse_login_response(Platform::Resonite, reqwest::StatusCode::OK, body).unwrap();
        let auth = session.to_authorization_info();
        assert_eq!(session.user_id.to_string(), "U-test");
        assert_eq!(auth.as_authorization_header_value(Platform::Resonite), "res U-test:test-token");
    }

    #[test]
    fn malformed_resonite_entity_reports_redacted_structure() {
        for body in [
            br#"{"entity":{"userId":"U-private","token":12345}}"#.as_slice(),
            br#"{"entity":{"userId":"U-private"}}"#.as_slice(),
            br#"{"entity":null}"#.as_slice(),
            br#"{"userId":"U-private","token":"private-token"}"#.as_slice(),
        ] {
            let error = parse_login_response(Platform::Resonite, reqwest::StatusCode::OK, body).err().unwrap().to_string();
            assert!(error.contains("200 OK"));
            assert!(error.contains("response structure"));
            for secret in ["U-private", "12345", "private-token"] {
                assert!(!error.contains(secret));
            }
        }
    }
}
