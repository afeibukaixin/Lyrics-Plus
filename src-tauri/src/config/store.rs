impl ConfigStore {
    pub fn load(app_dir: &Path, storage: &Storage) -> Result<(Self, bool), String> {
        let path = app_dir.join("config.json");
        if path.exists() {
            let raw =
                fs::read_to_string(&path).map_err(|error| format!("读取配置文件失败：{error}"))?;
            return match parse_config_draft(&raw) {
                Ok(parsed) => {
                    let comment_language =
                        detect_config_comment_language(&raw).unwrap_or_else(|| {
                            configured_comment_language(&parsed.config.app.language)
                        });
                    let source_raw = canonical_user_jsonc(&parsed.user, comment_language)?;
                    if raw != source_raw {
                        atomic_write(&path, &source_raw)?;
                    }
                    Ok((
                        Self {
                            path,
                            state: RwLock::new(ConfigStoreState {
                                value: parsed.config,
                                user: parsed.user,
                                revision: 1,
                                source_raw,
                                source_error: None,
                                comment_language,
                            }),
                        },
                        false,
                    ))
                }
                Err(error) => Ok((
                    Self {
                        path,
                        state: RwLock::new(ConfigStoreState {
                            value: AppConfig::default(),
                            user: Value::Object(Default::default()),
                            revision: 1,
                            source_raw: raw,
                            source_error: Some(error),
                            comment_language: UiLanguage::ZhCn,
                        }),
                    },
                    false,
                )),
            };
        }

        let mut value = AppConfig::default();
        value.app.player_selection = PlayerSelection::from_stored(
            storage.get_preference("player.selection").unwrap_or(None),
        );
        value.lyrics.providers = storage
            .get_preference("lyrics.providers")
            .unwrap_or(None)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        migrate_legacy_provider_order(&mut value.lyrics.providers);
        let bool_preference =
            |key: &str| storage.get_preference(key).unwrap_or(None).as_deref() == Some("true");
        if let Some(visible) = storage.get_preference("overlay.visible").unwrap_or(None) {
            value.lyrics.displays.desktop.enabled = visible == "true";
        }
        value.lyrics.displays.desktop.locked =
            bool_preference("overlay.locked") || bool_preference("overlay.passthrough");
        let last_monitor = storage
            .get_preference("overlay.last_monitor")
            .unwrap_or(None);
        let legacy_style = last_monitor
            .as_ref()
            .and_then(|id| {
                storage
                    .get_preference(&format!("overlay.style.{id}"))
                    .ok()
                    .flatten()
            })
            .or_else(|| {
                storage
                    .get_preference("overlay.style.default")
                    .ok()
                    .flatten()
            })
            .and_then(|raw| serde_json::from_str::<OverlayStyleSettings>(&raw).ok())
            .map(OverlayStyleSettings::normalized);
        if let Some(style) = legacy_style {
            // The legacy store only had one overlay style. Move its font
            // source to the common appearance before inheritance is applied,
            // so a user CSS stack is not replaced by the new default chain.
            value.lyrics.base_appearance.font_family = style.font_family.clone();
            value.lyrics.base_appearance.font_families = style.font_families.clone();
            value.lyrics.displays.desktop.apply_style(&style);
        }
        let value = value.normalized()?;
        let comment_language = configured_comment_language(&value.app.language);
        let user = validation::user_overrides_from_config(&value);
        let source_raw = canonical_user_jsonc(&user, comment_language)?;
        atomic_write(&path, &source_raw)?;
        Ok((
            Self {
                path,
                state: RwLock::new(ConfigStoreState {
                    value,
                    user,
                    revision: 1,
                    source_raw,
                    source_error: None,
                    comment_language,
                }),
            },
            true,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn snapshot(&self) -> AppConfig {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .value
            .clone()
    }

    #[cfg(test)]
    fn revision(&self) -> u64 {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .revision
    }

    pub fn editor_data(&self) -> ConfigEditorData {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let validation = if let Some(error) = &state.source_error {
            ConfigDraftValidation {
                valid: false,
                error: Some(error.clone()),
                normalized_json: None,
                effective_config: AppConfig::default(),
            }
        } else {
            validate_config_draft(&state.source_raw)
        };
        ConfigEditorData {
            default_jsonc: canonical_config_jsonc(&AppConfig::default(), state.comment_language)
                .expect("默认配置必须可以序列化"),
            user_json: state.source_raw.clone(),
            reset_jsonc: canonical_user_jsonc(
                &serde_json::json!({ "schemaVersion": CONFIG_SCHEMA_VERSION }),
                state.comment_language,
            )
            .expect("空用户配置必须可以序列化"),
            revision: state.revision,
            validation,
        }
    }

    pub fn replace(&self, value: AppConfig) -> Result<AppConfig, String> {
        self.replace_inner(value, None)
    }

    pub fn replace_at_revision(
        &self,
        value: AppConfig,
        expected_revision: u64,
    ) -> Result<AppConfig, String> {
        self.replace_inner(value, Some(expected_revision))
    }

    pub fn replace_draft(
        &self,
        value: AppConfig,
        user: Value,
        expected_revision: u64,
    ) -> Result<AppConfig, String> {
        let value = value.normalized()?;
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if expected_revision != state.revision {
            return Err(
                "config.conflict: configuration changed at another location; reload before saving"
                    .into(),
            );
        }
        self.replace_locked(&mut state, value, user)
    }

    pub fn reset_overrides(&self, pointers: &[&str]) -> Result<AppConfig, String> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let mut user = state.user.clone();
        validation::reset_user_overrides(&mut user, pointers);
        let value = validation::config_from_user_overrides(&user)?;
        self.replace_locked(&mut state, value, user)
    }

    fn replace_inner(
        &self,
        value: AppConfig,
        expected_revision: Option<u64>,
    ) -> Result<AppConfig, String> {
        let value = value.normalized()?;
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if expected_revision.is_some_and(|expected| expected != state.revision) {
            return Err(
                "config.conflict: configuration changed at another location; reload before saving"
                    .into(),
            );
        }
        let mut user = state.user.clone();
        // Diff against the last effective snapshot; for an invalid source the
        // snapshot is the default configuration and the override map is empty.
        validation::sync_user_overrides(&mut user, &state.value, &value);
        let effective = validation::config_from_user_overrides(&user)?;
        self.replace_locked(&mut state, effective, user)
    }

    fn replace_locked(
        &self,
        state: &mut ConfigStoreState,
        value: AppConfig,
        mut user: Value,
    ) -> Result<AppConfig, String> {
        if !user.is_object() {
            return Err("用户配置根节点必须是对象".into());
        }
        validation::prune_default_overrides(&mut user);
        user.as_object_mut()
            .expect("用户配置根节点必须是对象")
            .insert("schemaVersion".into(), Value::from(CONFIG_SCHEMA_VERSION));
        let raw = canonical_user_jsonc(&user, state.comment_language)?;
        atomic_write(&self.path, &raw)?;
        state.value = value.clone();
        state.user = user;
        state.source_raw = raw;
        state.source_error = None;
        state.revision = state.revision.saturating_add(1);
        Ok(value)
    }

    pub fn update(&self, edit: impl FnOnce(&mut AppConfig)) -> Result<AppConfig, String> {
        let mut next = self.snapshot();
        edit(&mut next);
        self.replace(next)
    }

    pub fn set_comment_language(&self, language: UiLanguage) -> Result<bool, String> {
        let language = match language {
            UiLanguage::ZhCn => UiLanguage::ZhCn,
            _ => UiLanguage::EnUs,
        };
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if state.comment_language == language {
            return Ok(false);
        }
        let localized = if state.source_error.is_none() {
            Some(canonical_user_jsonc(&state.user, language)?)
        } else {
            None
        };
        if let Some(raw) = localized.as_ref() {
            atomic_write(&self.path, raw)?;
        }
        state.comment_language = language;
        if let Some(raw) = localized {
            state.source_raw = raw;
        }
        state.revision = state.revision.saturating_add(1);
        Ok(true)
    }

    pub fn export_json(&self) -> Result<String, String> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        if state.source_error.is_some() {
            return Ok(state.source_raw.clone());
        }
        canonical_user_jsonc(&state.user, state.comment_language)
    }
}
fn atomic_write(path: &Path, raw: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "配置目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw).map_err(|error| format!("写入临时配置失败：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("替换配置文件失败：{error}"))
}
