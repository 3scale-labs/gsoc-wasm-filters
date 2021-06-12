use std::collections::HashMap;
use log::debug;
use threescalers::{
    api_call::{ApiCall, Kind},
    application::Application,
    extensions::{self},
    http::Request,
    credentials::*,
    service::Service,
    transaction::Transaction,
};

pub struct Auth {
    service_id: String,
    service_token: String,
    app_id: String,
}

impl Auth {
    pub fn service_id(&self) -> &str {
        self.service_id.as_str()
    }

    pub fn service_token(&self) -> &str {
        self.service_token.as_str()
    }

    pub fn app_id(&self) -> &str {
        self.app_id.as_str()
    }
}

pub fn auth_apps(service_key: String, app_keys: Vec<String>) -> Vec<Auth> {
    let keys = service_key.split('_').collect::<Vec<_>>();
    app_keys
        .iter()
        .map(|app_key| auth(keys[0].to_string(), keys[1].to_string(), app_key.clone()))
        .collect::<Vec<_>>()
}

pub fn auth(service_id: String, service_token: String, app_id: String) -> Auth {
    Auth {
        service_id,
        service_token,
        app_id,
    }
}

pub fn build_auth_request(auth: &Auth) -> Result<Request, anyhow::Error> {
    let creds = Credentials::ServiceToken(ServiceToken::from(auth.service_token()));
    let svc = Service::new(auth.service_id(), creds);
    let app = Application::from_user_key(auth.app_id());
    let txn = vec![(Transaction::new(&app, None, None, None))];
    let extensions = extensions::List::new().push(extensions::Extension::Hierarchy);
    let mut api_call = ApiCall::builder(&svc);
    let api_call = api_call
        .transactions(&txn)
        .extensions(&extensions)
        .kind(Kind::Authorize)
        .build()?;
    Ok(Request::from(&api_call))
}
