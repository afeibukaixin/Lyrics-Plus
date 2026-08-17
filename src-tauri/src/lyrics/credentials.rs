use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MusixmatchTokenType {
    DesktopUserToken,
    DeveloperApiKey,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ProviderCredentials {
    musixmatch_token: Option<String>,
    musixmatch_token_type: Option<MusixmatchTokenType>,
    musixmatch_anonymous_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialView {
    pub musixmatch_configured: bool,
    pub musixmatch_token_type: Option<MusixmatchTokenType>,
}

pub struct ProviderCredentialStore {
    path: Option<PathBuf>,
    value: RwLock<ProviderCredentials>,
}

impl ProviderCredentialStore {
    pub fn memory() -> Self {
        Self {
            path: None,
            value: RwLock::new(ProviderCredentials::default()),
        }
    }

    pub fn load(app_dir: &Path) -> Result<Self, String> {
        let path = app_dir.join("provider-credentials.json");
        let value = match fs::read_to_string(&path) {
            Ok(raw) => {
                restrict_permissions(&path)?;
                serde_json::from_str(&raw)
                    .map_err(|error| format!("无法读取歌词源凭据配置：{error}"))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ProviderCredentials::default()
            }
            Err(error) => return Err(format!("无法读取歌词源凭据配置：{error}")),
        };
        Ok(Self {
            path: Some(path),
            value: RwLock::new(value),
        })
    }

    pub fn view(&self) -> ProviderCredentialView {
        let credentials = self.musixmatch_credentials();
        ProviderCredentialView {
            musixmatch_configured: credentials.is_some(),
            musixmatch_token_type: credentials.map(|(token_type, _)| token_type),
        }
    }

    pub fn musixmatch_credentials(&self) -> Option<(MusixmatchTokenType, String)> {
        let value = self.value.read().unwrap_or_else(|error| error.into_inner());
        let token = value
            .musixmatch_token
            .clone()
            .filter(|token| !token.is_empty())?;
        // Credentials written before token types were introduced used the official API.
        let token_type = value
            .musixmatch_token_type
            .unwrap_or(MusixmatchTokenType::DeveloperApiKey);
        Some((token_type, token))
    }

    pub fn musixmatch_anonymous_token(&self) -> Option<String> {
        self.value
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .musixmatch_anonymous_token
            .clone()
            .filter(|token| !token.is_empty())
    }

    pub fn set_musixmatch_token(
        &self,
        token_type: MusixmatchTokenType,
        token: String,
    ) -> Result<ProviderCredentialView, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("Musixmatch API Token 不能为空".into());
        }
        self.update(|value| {
            value.musixmatch_token = Some(token.to_string());
            value.musixmatch_token_type = Some(token_type);
        })?;
        Ok(self.view())
    }

    pub fn clear_musixmatch_token(&self) -> Result<ProviderCredentialView, String> {
        self.update(|value| {
            value.musixmatch_token = None;
            value.musixmatch_token_type = None;
        })?;
        Ok(self.view())
    }

    pub fn set_musixmatch_anonymous_token(&self, token: String) -> Result<(), String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("Musixmatch 匿名 Token 不能为空".into());
        }
        self.update(|value| value.musixmatch_anonymous_token = Some(token.to_string()))
    }

    pub fn clear_musixmatch_anonymous_token(&self) -> Result<(), String> {
        self.update(|value| value.musixmatch_anonymous_token = None)
    }

    fn update(&self, change: impl FnOnce(&mut ProviderCredentials)) -> Result<(), String> {
        let mut value = self
            .value
            .write()
            .unwrap_or_else(|error| error.into_inner());
        change(&mut value);
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建歌词源凭据目录：{error}"))?;
        }
        let raw = serde_json::to_string_pretty(&*value)
            .map_err(|error| format!("无法序列化歌词源凭据：{error}"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, raw).map_err(|error| format!("无法保存歌词源凭据：{error}"))?;
        restrict_permissions(&temporary)?;
        fs::rename(&temporary, path).map_err(|error| format!("无法替换歌词源凭据配置：{error}"))?;
        Ok(())
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("无法限制歌词源凭据权限：{error}"))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
