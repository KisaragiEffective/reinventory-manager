use reqwest::header::AUTHORIZATION;
use log::{debug, warn};
use crate::model::{AuthorizationInfo, DirectoryMetadata, LoginResponse, Record, UserId, UserLoginPostBody, UserLoginPostResponse};

pub struct Operation;

static BASE_POINT: &str = "https://api.neos.com/api";

impl Operation {
    pub async fn login() -> Option<LoginResponse> {
        let client = reqwest::Client::new();
        debug!("post");
        if let Some(auth) = &crate::get_args_lock().login_info {
            let email = auth.email.clone();
            let password = auth.password.clone();
            let token_res = client
                .post(format!("{BASE_POINT}/userSessions"))
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

    pub async fn logout(owner_id: UserId, authorization_info: AuthorizationInfo) {
        let client = reqwest::Client::new();
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

    pub async fn move_record(owner_id: UserId, record: Record, from: Vec<String>, to: Vec<String>, authorization_info: &Option<AuthorizationInfo>) {
        let client = reqwest::Client::new();
        let new_path = to.join("/");
        let find = Self::get_directory_items(owner_id.clone(), from, authorization_info).await.into_iter().find(|a| a.id == record.id);

        if let Some(found_record) = find {
            // region delete old record
            {
                let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = record.id);
                let mut req = client.delete(endpoint);

                if let Some(authorization_info) = authorization_info {
                    req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
                }

                let deleted = req
                    .send()
                    .await
                    .unwrap()
                    .json::<Record>()
                    .await
                    .unwrap();

                debug!("deleted: {deleted:?}");
            }
            // endregion
            // region insert
            {
                let endpoint = format!("{BASE_POINT}/users/{owner_id}/records/{record_id}", owner_id = &owner_id, record_id = &record.id);
                debug!("endpoint: {endpoint}", endpoint = &endpoint);
                // * もしエラーが起きたらGUIDを新しく割り当てる
                let mut req = client.put(endpoint);

                if let Some(authorization_info) = authorization_info {
                    req = req.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
                }

                req
                    .json(&record)
                    .send()
                    .await
                    .unwrap();
            }
            // endregion
        } else {
            warn!("not found");
        }
    }
}
