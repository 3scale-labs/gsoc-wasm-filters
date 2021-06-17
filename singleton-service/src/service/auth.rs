use log::debug;
use threescale::structs::AppIdentifier;
use threescalers::{
    api_call::{ApiCall, Kind},
    application::Application,
    credentials::*,
    extensions::{self},
    http::Request,
    service::Service,
    transaction::Transaction,
};

/// Proxy level representation for authorization request.
pub struct Auth {
    service_id: String,
    service_token: String,
    app_id: AppIdentifier,
}

impl Auth {
    pub fn service_id(&self) -> &str {
        self.service_id.as_str()
    }

    pub fn service_token(&self) -> &str {
        self.service_token.as_str()
    }

    pub fn app_id(&self) -> &AppIdentifier {
        &self.app_id
    }
}

/// Create a vector of Auth objects for a service. Take service_key(service_id + service_token)
/// and apps_keys of type Vec<AppIdentifier> as arguments to the function and returns Vec<Auth>.
pub fn auth_apps(service_key: String, app_keys: Vec<AppIdentifier>) -> Vec<Auth> {
    let keys = service_key.split('_').collect::<Vec<_>>();
    app_keys
        .iter()
        .map(|app| auth(keys[0].to_string(), keys[1].to_string(), app.clone()))
        .collect::<Vec<_>>()
}

/// Create a Auth object for an application. Take service_id, service_token and app_id of type
/// AppIdentifier and returns an Auth object.
pub fn auth(service_id: String, service_token: String, app_id: AppIdentifier) -> Auth {
    Auth {
        service_id,
        service_token,
        app_id,
    }
}

/// Create a Request of type Authorize. Take an object of type Auth as argument to the function
/// and returns Result<Request, anyhow::Error>.
pub fn build_auth_request(auth: &Auth) -> Result<Request, anyhow::Error> {
    let creds = Credentials::ServiceToken(ServiceToken::from(auth.service_token()));
    let svc = Service::new(auth.service_id(), creds);
    let app;
    match auth.app_id() {
        AppIdentifier::UserKey(user_key) => {
            debug!("AppIdentifier UserKey: {:?}", user_key);
            app = Application::from_user_key(user_key.as_ref())
        }
        AppIdentifier::AppId(app_id, None) => {
            debug!("AppIdentifier AppId : {:?}", app_id);
            app = Application::from_app_id(app_id.as_ref())
        }
        AppIdentifier::AppId(app_id, Some(app_key)) => {
            debug!("AppIdentifier AppId+AppKey : {:?}_{:?}", app_id, app_key);
            app = Application::from_app_id_and_key(app_id.as_ref(), app_key.as_ref())
        }
    }
    let txn = vec![(Transaction::new(&app, None, None, None))];
    // TODO : Enable list keys extension.
    let extensions = extensions::List::new().push(extensions::Extension::Hierarchy);
    let mut api_call = ApiCall::builder(&svc);
    let api_call = api_call
        .transactions(&txn)
        .extensions(&extensions)
        .kind(Kind::Authorize)
        .build()?;
    Ok(Request::from(&api_call))
}
