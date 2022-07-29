use reqwest::header::AUTHORIZATION;
use log::debug;
use crate::model::{AuthorizationInfo, LoginResponse, PathPointedRecordResponse, UserId, UserLoginPostBody, UserLoginPostResponse};

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

    pub async fn get_record_at_path(owner_id: UserId, path: Vec<String>, authorization_info: &Option<AuthorizationInfo>) -> Vec<PathPointedRecordResponse> {
        let client = reqwest::Client::new();
        let path = path.join("%5C");
        // NOTE:
        // https://api.neos.com/api/users/U-kisaragi-marine/records/root/Inventory/Test <-- これはディレクトリのメタデータを単体で返す


        // NOTE: これはドキュメントが古い。 (thanks kazu)
        // あと、Personalから始めるのではなく、Inventoryから先頭のバックスラッシュなしでパスを指定する点に注意
        let endpoint = if path.is_empty() {
            format!("{BASE_POINT}/users/{owner_id}/records/root")
        } else {
            format!("{BASE_POINT}/users/{owner_id}/records?path={path}")
        };

        debug!("endpoint: {endpoint}", endpoint = &endpoint);
        let mut res = client.get(endpoint);

        if let Some(authorization_info) = authorization_info {
            res = res.header(reqwest::header::AUTHORIZATION, authorization_info.as_authorization_header_value());
        }

        let res = res
            .send()
            .await
            .unwrap()
            .json::<Vec<PathPointedRecordResponse>>()
            .await
            .unwrap();

        res
    }
}
