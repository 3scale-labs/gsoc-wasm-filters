use crate::structs::{AppId, AppIdentifier, Application, ServiceId, UserKey};
use log::{debug, info};
use proxy_wasm::hostcalls::{get_shared_data, set_shared_data};
use std::hash::{Hash, Hasher};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("app_id not found in the cache")]
    AppIdNotFound,
    #[error("application not found in the cache")]
    AppNotFound,
    #[error("[u8] to str conversion failed")]
    Utf8Fail(#[from] std::str::Utf8Error),
    #[error("failure caused by an underlying proxy issue")]
    ProxyStatus(u8),
    #[error("deserializing usage data failed")]
    DeserializeFail(#[from] bincode::ErrorKind),
}

#[derive(Debug, Clone, Eq)]
pub struct CacheKey(ServiceId, AppIdentifier);

impl<'a> CacheKey {
    pub fn as_string(&self) -> String {
        format!("{}_{}", self.0.as_ref(), self.1.as_ref())
    }

    pub fn service_id(&'a self) -> &'a ServiceId {
        &self.0
    }

    pub fn app_id(&'a self) -> &'a AppIdentifier {
        &self.1
    }

    pub fn default() -> Self {
        Self(ServiceId::from(""), AppIdentifier::appid_from_str(""))
    }
    pub fn set_app_id(&'a mut self, new_app_id: &AppIdentifier) {
        self.1 = new_app_id.clone()
    }

    pub fn from(a: &ServiceId, b: &AppIdentifier) -> CacheKey {
        CacheKey {
            0: a.clone(),
            1: b.clone(),
        }
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_string().hash(state);
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_string() == other.as_string()
    }
}

// Returns Application from shared data with CAS integer
pub fn get_application_from_cache(key: &CacheKey) -> Result<(Application, u32), CacheError> {
    match get_shared_data(&key.as_string()) {
        Ok((Some(bytes), Some(cas))) => match bincode::deserialize::<Application>(&bytes) {
            Ok(app) => Ok((app, cas)),
            Err(e) => Err(CacheError::DeserializeFail(*e)),
        },
        Ok((_bytes, _cas)) => Err(CacheError::AppNotFound),
        Err(e) => Err(CacheError::ProxyStatus(e as u8)),
    }
}

pub fn get_app_id_from_cache(user_key: &UserKey) -> Result<AppId, CacheError> {
    match get_shared_data(user_key.as_ref()) {
        Ok((Some(bytes), _cas)) => Ok(AppId::from(std::str::from_utf8(&bytes)?)),
        Ok((None, _cas)) => Err(CacheError::AppIdNotFound),
        Err(e) => Err(CacheError::ProxyStatus(e as u8)),
    }
}

// overwrites if already present inside the cache
pub fn set_app_id_to_cache(user_key: &UserKey, app_id: &AppId) -> Result<(), CacheError> {
    if let Err(e) = set_shared_data(user_key.as_ref(), Some(app_id.as_ref().as_bytes()), None) {
        return Err(CacheError::ProxyStatus(e as u8));
    }
    Ok(())
}

// if cas is 0, cache record is overwritten
// returns false on set failure
pub fn set_application_to_cache(key: &str, app: &Application, cas: u32) -> bool {
    info!("setting application with key: {}", key);
    match set_shared_data(
        key,
        Some(&bincode::serialize::<Application>(app).unwrap()),
        Some(cas),
    ) {
        Ok(()) => true,
        Err(e) => {
            debug!("set operation failed for key: {} : {:?}", key, e);
            false
        }
    }
}
