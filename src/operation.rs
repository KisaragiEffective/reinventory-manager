use std::sync::Arc;
use reqwest::header::AUTHORIZATION;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use reqwest::{Client, ClientBuilder};
use uuid::Uuid;
use crate::LoginInfo;
use crate::model::{AuthorizationInfo, DirectoryMetadata, AbsoluteInventoryPath, Record, RecordId, RecordType, UserId, UserLoginPostBody, UserLoginPostResponse};

static BASE_POINT: &str = "https://api.neos.com/api";
static CLIENT: Lazy<Arc<Client>> = Lazy::new(|| Arc::new(
    ClientBuilder::new().user_agent("NeosVR-Inventory-Manager/0.1")
        .use_rustls_tls()
        .build()
        .expect("failed to initialize HTTP client")
));

pub struct PreLogin;

impl PreLogin {
    pub async fn login(login_info: Option<LoginInfo>) -> LoggedIn {
        if let Some(auth) = login_info {
            let mut req = CLIENT
                .post(format!("{BASE_POINT}/userSessions"));

            if let Some(x) = auth.get_totp() {
                req = req.header("TOTP", x.0.clone());
            }

            let token_res = req
                .json(&UserLoginPostBody::create(auth, false))
                .send();

            debug!("post 2");
            let token_res = token_res
                .await
                .unwrap()
                .json::<UserLoginPostResponse>()
                .await
                .unwrap();

            debug!("post 3");
            let using_token = token_res.to_authorization_info();
            let user_id = token_res.user_id;

            debug!("post 4");
            Self::from_session_data(Some(user_id), Some(using_token))
        } else {
            Self::from_session_data(None, None)
        }
    }

    pub const fn from_session_data(current_user: Option<UserId>, authorization_info: Option<AuthorizationInfo>) -> LoggedIn {
        LoggedIn {
            authorization_info,
            current_user,
        }
    }
}

pub struct LoggedIn {
    authorization_info: Option<AuthorizationInfo>,
    current_user: Option<UserId>,
}

impl LoggedIn {
    pub async fn logout(self) {
        if let Some(authorization_info) = self.authorization_info {
            let owner_id = authorization_info.owner_id.clone();
            CLIENT
                .delete(format!("{BASE_POINT}/userSessions/{owner_id}/{auth_token}", auth_token = authorization_info.token))
                .header(AUTHORIZATION, authorization_info.as_authorization_header_value())
                .send()
                .await
                .unwrap();
        }
    }

    pub async fn get_directory_items(&self, owner_id: UserId, path: AbsoluteInventoryPath) -> Vec<Record> {
        let authorization_info = &self.authorization_info;
        let path = path.to_uri_query_value();
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す


        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records?path={path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        {
            let mut res = CLIENT.get(&endpoint);

            if let Some(authorization_info) = authorization_info {
                res = res.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
            }

            let res = res
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            debug!("raw output: {res}");
        }
        let mut res = CLIENT.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        res
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn get_directory_metadata(&self, owner_id: UserId, path: AbsoluteInventoryPath) -> DirectoryMetadata {
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す
        let authorization_info = &self.authorization_info;
        let path = path.to_absolute_path();
        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/root/{path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = CLIENT.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        res
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn move_records(&self, owner_id: UserId, records_to_move: Vec<RecordId>, to: Vec<String>, keep_record_id: bool) {
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
                    let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id);
                    let mut req = CLIENT.delete(endpoint);

                    if let Some(authorization_info) = authorization_info {
                        req = req.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
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

                    let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);
                    debug!("endpoint: {endpoint}", endpoint = &endpoint);
                    let mut request = CLIENT.put(endpoint);

                    if let Some(authorization_info) = authorization_info {
                        debug!("auth set");
                        request = request.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
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
        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);

        let mut request = CLIENT
            .get(endpoint);

        if let Some(authorization_info) = &self.authorization_info {
            debug!("auth set");
            request = request.header(AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = request
            .send()
            .await
            .expect("HTTP connection error");

        match res.status().as_u16() {
            200 => {
                let record = res
                    .json()
                    .await
                    .expect("Failed to parse JSON: This is critical bug. Please open ticket on https://github.com/KisaragiEffective/neosvr-inventory-management/issues.");

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
