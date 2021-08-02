use crate::structs::{AppId, AppIdentifier, Application, ServiceId, UserKey};
use log::{debug, info, warn};
use proxy_wasm::hostcalls::{get_shared_data, set_shared_data};
use std::convert::TryInto;
use std::hash::{Hash, Hasher};

pub const SHARED_MEMORY_KEY: &str = "SHARED_MEMORY_COUNTER";
pub const SHARED_MEMORY_INITIAL_SIZE: u64 = 1000;

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
    #[error("deserializing bincode data failed")]
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
    let prev_memory_usage = get_cache_pair_size(key) as i32;
    let serialized_app = bincode::serialize::<Application>(app).unwrap();
    let memory_delta: i32 = (key.len() as i32) + (serialized_app.len() as i32) - prev_memory_usage;
    if let Err(e) = set_shared_data(key, Some(&serialized_app), Some(cas)) {
        debug!("set operation failed for key: {} : {:?}", key, e);
        return false;
    }
    for num_try in 0..3 {
        match update_shared_memory_size(memory_delta) {
            Ok(()) => break,
            Err(e) => debug!("try#{} : failed to update memory counter: {}", num_try, e),
        }
    }
    true
}

// returns memory used in bytes for both key and value pair stored.
fn get_cache_pair_size(key: &str) -> usize {
    match get_shared_data(key) {
        Ok((Some(bytes), _)) => key.len() + bytes.len(),
        Ok((None, _)) => key.len(),
        Err(_) => 0,
    }
}

// Adds delta bytes to the shared memory usage counter
fn update_shared_memory_size(delta: i32) -> Result<(), anyhow::Error> {
    let current_size = match get_shared_data(SHARED_MEMORY_KEY) {
        Ok((Some(bytes), _)) => {
            let arr: [u8; 8] = match bytes.try_into() {
                Ok(res) => res,
                Err(e) => anyhow::bail!("failed to convert vec<u8> to [u8;8]: {:?}", e),
            };
            u64::from_be_bytes(arr)
        }
        Ok((None, _)) => {
            warn!("shared memory size was not initialized at the start or got deleted somehow!");
            if let Err(e) = set_shared_data(
                SHARED_MEMORY_KEY,
                Some(&SHARED_MEMORY_INITIAL_SIZE.to_be_bytes()),
                None,
            ) {
                anyhow::bail!(
                    "failed to initialize shared memory size: {:?}",
                    CacheError::ProxyStatus(e as u8)
                )
            }
            SHARED_MEMORY_INITIAL_SIZE
        }
        Err(e) => anyhow::bail!(
            "getting shared memory size failed: {:?}",
            CacheError::ProxyStatus(e as u8)
        ),
    };

    let final_size: u64;
    if delta.is_negative() {
        if current_size <= (-delta).try_into().unwrap() {
            // This condition is theoretically not possible because memory should keep on
            // increasing since we have no option of deleting the data. So, check your calcs!
            final_size = SHARED_MEMORY_INITIAL_SIZE;
        } else {
            final_size = current_size.saturating_sub((-delta).try_into().unwrap());
        }
    } else {
        final_size = current_size.saturating_add((-delta).try_into().unwrap());
    }

    if let Err(e) = set_shared_data(SHARED_MEMORY_KEY, Some(&final_size.to_be_bytes()), None) {
        anyhow::bail!(
            "failed to update shared memory size: {:?}",
            CacheError::ProxyStatus(e as u8)
        )
    }
    Ok(())
}
