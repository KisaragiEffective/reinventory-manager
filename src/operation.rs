use reqwest::header::AUTHORIZATION;
use log::{debug, error, info, warn};
use async_recursion::async_recursion;
use chrono::Utc;
use reqwest::Client;
use uuid::Uuid;
use crate::LoginInfo;
use crate::model::{AuthorizationInfo, DirectoryMetadata, LoginResponse, Record, RecordId, RecordOwner, RecordType, SessionState, UserId, UserLoginPostBody, UserLoginPostResponse};

pub struct Operation;

static BASE_POINT: &str = "https://api.neos.com/api";

impl Operation {
    pub async fn login(login_info: Option<LoginInfo>) -> Option<LoginResponse> {
        let client = reqwest::Client::new();
        debug!("post");
        if let Some(auth) = login_info {
            let mut req = client
                .post(format!("{BASE_POINT}/userSessions"));

            if let Some(x) = auth.get_totp() {
                req = req.header("TOTP", x.0.clone())
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
            let using_token = (&token_res).to_authorization_info();
            let user_id = token_res.user_id;

            debug!("post 4");
            Some(LoginResponse {
                using_token,
                user_id,
            })
        } else {
            None
        }
    }

    pub async fn logout(authorization_info: AuthorizationInfo) {
        let client = reqwest::Client::new();
        let owner_id = authorization_info.owner_id.clone();
        client
            .delete(format!("{BASE_POINT}/userSessions/{owner_id}/{auth_token}", auth_token = authorization_info.token))
            .header(AUTHORIZATION, authorization_info.as_authorization_header_value())
            .send()
            .await
            .unwrap();
    }

    pub async fn get_directory_items(owner_id: UserId, path: Vec<String>, authorization_info: &Option<AuthorizationInfo>) -> Vec<Record> {
        let client = reqwest::Client::new();
        let path = path.join("%5C");
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す


        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records?path={path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        {
            let mut res = client.get(&endpoint);

            if let Some(authorization_info) = authorization_info {
                res = res.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
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
        let mut res = client.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = res
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        res
    }

    pub async fn get_directory_metadata(owner_id: UserId, path: Vec<String>, authorization_info: &Option<AuthorizationInfo>) -> DirectoryMetadata {
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す
        let client = reqwest::Client::new();
        let path = path.join("/");
        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/root/{path}");

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = client.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = res
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        res
    }

    #[async_recursion]
    pub async fn move_record(
        owner_id: UserId,
        record_id: RecordId,
        new_base_directory: Vec<String>,
        authorization_info: &Option<AuthorizationInfo>,
        keep_record_id: bool,
        session_state: SessionState
    ) {
        let client = reqwest::Client::new();
        let find = Self::get_record(owner_id.clone(), record_id.clone(), authorization_info).await;

        if let Some(found_record) = find {
            debug!("found, moving");

            let from = (&found_record.path).clone();

            // region insert
            {
                debug!("insert!");
                if found_record.record_type == RecordType::Directory {
                    Self::create_directory(owner_id.clone(), new_base_directory.clone(), found_record.name.clone(), authorization_info, session_state.clone()).await;
                    let dig_into_dir = {
                        let mut d = new_base_directory.clone();
                        d.push(found_record.name.clone());
                        d
                    };
                    let child_items = Self::get_directory_items(owner_id.clone(), from.split('\\').map(|a| a.to_string()).collect(), authorization_info).await;
                    for child in child_items {
                        // FIXME: infinite recursion
                        Self::move_record(
                            owner_id.clone(),
                            child.id,
                            dig_into_dir.clone(),
                            authorization_info,
                            keep_record_id,
                            session_state.clone()
                        ).await;
                    }
                } else {
                    let record_id = {
                        // GUIDは小文字が「推奨」されているため念の為小文字にしておく
                        let record_id = RecordId(format!("R-{}", Uuid::new_v4().to_string().to_lowercase()));
                        debug!("new record id: {record_id}", record_id = &record_id);
                        record_id
                    };

                    let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);
                    debug!("endpoint: {endpoint}", endpoint = &endpoint);
                    let mut req = client.put(endpoint);

                    if let Some(authorization_info) = authorization_info {
                        debug!("auth set");
                        req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
                    }

                    let mut record = found_record.clone();
                    record.path = new_base_directory.join("\\");
                    record.id = record_id.clone();

                    debug!("requesting...");
                    let res = req
                        .json(&record)
                        .send()
                        .await
                        .unwrap();
                    if res.status().is_success() {
                        info!("Success! {record_id} for {owner_id} was moved from {from} to {to}.", to = new_base_directory.join("\\"), record_id = &record_id);
                    } else if res.status().is_client_error() {
                        error!("Client error ({status}): this is fatal bug. Please report this to bug tracker.", status = res.status());
                    } else if res.status().is_server_error() {
                        error!("Server error ({status}): Please try again in later.", status = res.status());
                    } else {
                        warn!("Unhandled status code: {status}", status = res.status())
                    }
                    debug!("Response: {res:?}", res = &res);
                }
            }
            // endregion
            // region delete old record
            {
                let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id);
                let mut req = client.delete(endpoint);

                if let Some(authorization_info) = authorization_info {
                    req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
                }

                let deleted = req
                    .send()
                    .await
                    .unwrap();

                debug!("deleted: {deleted:?}");
            }
            // endregion
        } else {
            warn!("not found");
        }
    }

    pub async fn get_record(owner_id: UserId, record_id: RecordId, authorization_info: &Option<AuthorizationInfo>) -> Option<Record> {
        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);
        let client = reqwest::Client::new();

        let mut req = client
            .get(endpoint);

        if let Some(authorization_info) = authorization_info {
            debug!("auth set");
            req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = req
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

    pub async fn insert_record(owner_id: UserId, path: Vec<String>, mut record: Record, authorization_info: &Option<AuthorizationInfo>) {
        debug!("Preparing insert record for {owner_id}, to {path}. Content: {record:?}", owner_id = &owner_id, path = &path.join("\\"));
        let new_record_id = {
            // GUIDは小文字が「推奨」されているため念の為小文字にしておく
            let record_id = RecordId(format!("R-{}", Uuid::new_v4().to_string().to_lowercase()));
            debug!("new record id: {record_id}", record_id = &record_id);
            record_id
        };

        let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &new_record_id);
        record.id = new_record_id.clone();

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let client = Client::new();
        let mut req = client.put(endpoint);

        if let Some(authorization_info) = authorization_info {
            debug!("auth set");
            req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        debug!("requesting...");
        let res = req
            .json(&record)
            .send()
            .await
            .unwrap();
        if res.status().is_success() {
            info!("Success! Created record with {new_record_id} for {owner_id}. Content: {record:?}");
        } else if res.status().is_client_error() {
            error!("Client error ({status}): this is fatal bug. Please report this to bug tracker.", status = res.status());
        } else if res.status().is_server_error() {
            error!("Server error ({status}): Please try again in later.", status = res.status());
        } else {
            warn!("Unhandled status code: {status}", status = res.status())
        }
        debug!("Response: {res:?}", res = &res);
    }

    pub async fn create_directory(owner_id: UserId, base_dir: Vec<String>, name: String, authorization_info: &Option<AuthorizationInfo>, session: SessionState) -> RecordId {
        let created_date = Utc::now();
        let id = RecordId::make_random();
        Self::insert_record(owner_id.clone(), {
            let mut d = base_dir.clone();
            d.push(name.clone());
            d
        }, Record {
            id: id.clone(),
            asset_uri: None,
            global_version: 0,
            local_version: 0,
            last_update_by: Some(owner_id.clone()),
            last_update_machine: Some(session.machine_id),
            name,
            record_type: RecordType::Directory,
            owner_name: Some(session.user_name),
            tags: vec![],
            path: base_dir.join("\\"),
            is_public: false,
            is_for_patrons: false,
            is_listed: false,
            is_deleted: false,
            thumbnail_uri: None,
            created_at: Some(created_date),
            updated_at: created_date,
            random_order: 0,
            visits: 0,
            rating: 0.0,
            owner_id: Some(RecordOwner::User(owner_id)),
            submissions: vec![]
        }, authorization_info).await;

        id
    }

    pub async fn lookup_user_name(id: UserId) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct PartialUserResponse {
            #[serde(rename = "username")]
            user_name: String
        }

        // see: https://github.com/PolyLogiX-Studio/NeosVR-API/issues/6
        let endpoint = format!("{BASE_POINT}/users/{id}?byUsername=false");
        let client = Client::new();
        let res = client.get(endpoint)
            .send()
            .await
            .expect("HTTP connection issue");

        let status_code = res.status();

        match status_code.as_u16() {
            200 => {
                let v = res
                    .json::<PartialUserResponse>()
                    .await
                    .expect("Failed to deserialize response")
                    .user_name;

                Some(v)
            }
            404 => {
                error!("User not found");
                None
            }
            other => {
                warn!("Unhandled: {other}");
                None
            }
        }
    }
}
