use reqwest::header::AUTHORIZATION;
use log::{debug, error, info, warn};
use async_recursion::async_recursion;
use uuid::Uuid;
use crate::model::{AuthorizationInfo, DirectoryMetadata, LoginResponse, Record, RecordId, RecordType, UserId, UserLoginPostBody, UserLoginPostResponse};

pub struct Operation;

static BASE_POINT: &str = "https://api.neos.com/api";

impl Operation {
    pub async fn login() -> Option<LoginResponse> {
        let client = reqwest::Client::new();
        debug!("post");
        if let Some(auth) = &crate::get_args_lock().login_info {
            let email = auth.email.clone();
            let password = auth.password.clone();
            let mut req = client
                .post(format!("{BASE_POINT}/userSessions"));

            if let Some(x) = &auth.totp {
                req = req.header("TOTP", x.0.clone())
            }

            let token_res = req
                .json(&UserLoginPostBody::create(email, password, false))
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
    pub async fn find_recursive(base_dir: Vec<String>, owner_id: UserId, record_id: RecordId, authorization_info: &Option<AuthorizationInfo>) -> Option<Record> {
        for record in Self::get_directory_items(owner_id.clone(), base_dir.clone(), authorization_info).await {
            debug!("checking '{name}' (type={tp} | {id})", name = &record.name, tp = &record.record_type, id = &record.id);

            if record.id == record_id {
                return Some(record)
            } else if record.record_type == RecordType::Directory {
                let opt = Self::find_recursive({
                    let mut buf = base_dir.clone();
                    buf.push(record.name);
                    buf
                }, owner_id.clone(), record_id.clone(), authorization_info).await;
                if let Some(found) = opt {
                    return Some(found)
                }
            }
        }

        debug!("---------------------------------");

        None
    }

    pub async fn move_record(owner_id: UserId, record_id: RecordId, to: Vec<String>, authorization_info: &Option<AuthorizationInfo>) {
        let client = reqwest::Client::new();
        let find = Self::find_recursive(vec!["Inventory".to_string()], owner_id.clone(), record_id.clone(), authorization_info).await;

        if let Some(found_record) = find {
            debug!("found, moving");

            let from = (&found_record.path).clone();

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
            // region insert
            {
                debug!("insert!");
                // GUIDは小文字が「推奨」されているため念の為小文字にしておく
                let record_id = RecordId(format!("R-{}", Uuid::new_v4().to_string().to_lowercase()));
                debug!("new record id: {record_id}", record_id = &record_id);
                let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record_id);
                debug!("endpoint: {endpoint}", endpoint = &endpoint);
                let mut req = client.put(endpoint);

                if let Some(authorization_info) = authorization_info {
                    debug!("auth set");
                    req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
                }

                let mut record = found_record.clone();
                record.path = to.join("\\");
                record.id = record_id.clone();

                debug!("requesting...");
                let res = req
                    .json(&record)
                    .send()
                    .await
                    .unwrap();
                if res.status().is_success() {
                    info!("Success! {record_id} for {owner_id} was moved from {from} to {to}.", to = to.join("\\"));
                } else if res.status().is_client_error() {
                    error!("Client error ({status}): this is fatal bug. Please report this to bug tracker.", status = res.status());
                    // TODO: rollback
                } else if res.status().is_server_error() {
                    error!("Server error ({status}): Please try again in later.", status = res.status());
                } else {
                    warn!("Unhandled status code: {status}", status = res.status())
                }
                debug!("Response: {res:?}", res = &res);
            }
            // endregion
        } else {
            warn!("not found");
        }
    }
}
