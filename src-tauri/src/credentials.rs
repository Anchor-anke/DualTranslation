use crate::error::{AppError, AppResult};
use keyring::Entry;

const SERVICE: &str = "com.dualtranslation.desktop.provider";

pub fn save(profile_id: &str, api_key: &str) -> AppResult<()> {
    if api_key.trim().is_empty() {
        return Err(AppError::new(
            "CREDENTIAL_UNAVAILABLE",
            "API Key 不能为空。",
            "请输入有效密钥后再保存。",
        ));
    }
    let entry = Entry::new(SERVICE, profile_id).map_err(credential_error)?;
    entry.set_password(api_key).map_err(credential_error)
}

pub fn get(profile_id: &str) -> AppResult<String> {
    let entry = Entry::new(SERVICE, profile_id).map_err(credential_error)?;
    entry.get_password().map_err(credential_error)
}

fn credential_error(_: keyring::Error) -> AppError {
    AppError::new(
        "CREDENTIAL_UNAVAILABLE",
        "系统凭据库不可用，未保存或读取 API Key。",
        "请确认系统钥匙串或凭据管理器可用后重试；应用不会回退为明文存储。",
    )
}
