use std::collections::{BTreeMap, HashSet};

use aionui_common::encrypt_string;
use aionui_db::{
    CreateMcpServerParams, CreateProviderParams, IAssistantDefinitionRepository, IMcpServerRepository,
    IProviderRepository, SqliteAssistantDefinitionRepository, SqliteMcpServerRepository, SqliteProviderRepository,
    UpdateMcpServerParams, UpdateProviderParams, UpsertAssistantDefinitionParams,
};
use anyhow::{Context, bail};
use serde::Deserialize;

use crate::config::derive_encryption_key;
use crate::services::AppServices;

const DEFAULT_USER_ID: &str = "system_default_user";
const PRESET_VERSION: u32 = 2;
const PRESET_JSON: &str = include_str!("../assets/preset/computing-platform.json");

#[derive(Debug, Deserialize)]
struct PresetConfiguration {
    version: u32,
    provider: ProviderPreset,
    mcp: McpPreset,
    assistant: AssistantPreset,
}

#[derive(Debug, Deserialize)]
struct ProviderPreset {
    id: String,
    platform: String,
    name: String,
    base_url: String,
    models: Vec<ModelPreset>,
    enabled: bool,
    #[serde(default)]
    is_full_url: bool,
}

#[derive(Debug, Deserialize)]
struct ModelPreset {
    id: String,
    protocol: String,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct McpPreset {
    name: String,
    description: String,
    url: String,
    enabled: bool,
    #[serde(default)]
    headers: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AssistantPreset {
    id: String,
    default_model: String,
}

struct ProviderPayloads {
    models: String,
    model_protocols: String,
    model_enabled: String,
}

fn enabled_by_default() -> bool {
    true
}

fn load_preset() -> anyhow::Result<PresetConfiguration> {
    load_preset_from_str(PRESET_JSON)
}

fn load_preset_from_str(raw: &str) -> anyhow::Result<PresetConfiguration> {
    let config: PresetConfiguration = serde_json::from_str(raw).context("parse computing platform preset")?;
    validate_preset(&config)?;
    Ok(config)
}

fn validate_preset(config: &PresetConfiguration) -> anyhow::Result<()> {
    if config.version != PRESET_VERSION {
        bail!(
            "unsupported computing platform preset version {}; expected {PRESET_VERSION}",
            config.version
        );
    }
    if config.provider.id.trim().is_empty()
        || config.provider.platform.trim().is_empty()
        || config.provider.name.trim().is_empty()
        || config.provider.base_url.trim().is_empty()
    {
        bail!("computing platform provider identity and endpoint must not be empty");
    }
    if config.mcp.name.trim().is_empty() || config.mcp.url.trim().is_empty() {
        bail!("computing platform MCP name and endpoint must not be empty");
    }
    if config.assistant.id.trim().is_empty() || config.assistant.default_model.trim().is_empty() {
        bail!("computing platform assistant id and default model must not be empty");
    }
    if config.provider.models.is_empty() {
        bail!("computing platform preset requires at least one model");
    }

    let mut model_ids = HashSet::new();
    for model in &config.provider.models {
        if model.id.trim().is_empty() || model.protocol.trim().is_empty() {
            bail!("computing platform model id and protocol must not be empty");
        }
        if !model_ids.insert(model.id.as_str()) {
            bail!("computing platform model ids must be unique");
        }
    }

    let Some(default_model) = config
        .provider
        .models
        .iter()
        .find(|model| model.id == config.assistant.default_model)
    else {
        bail!("assistant.default_model must exist in provider.models");
    };
    if !default_model.enabled {
        bail!("assistant.default_model must be enabled");
    }
    Ok(())
}

fn provider_payloads(config: &ProviderPreset) -> anyhow::Result<ProviderPayloads> {
    let models: Vec<&str> = config.models.iter().map(|model| model.id.as_str()).collect();
    let model_protocols: BTreeMap<&str, &str> = config
        .models
        .iter()
        .map(|model| (model.id.as_str(), model.protocol.as_str()))
        .collect();
    let model_enabled: BTreeMap<&str, bool> = config
        .models
        .iter()
        .map(|model| (model.id.as_str(), model.enabled))
        .collect();
    Ok(ProviderPayloads {
        models: serde_json::to_string(&models)?,
        model_protocols: serde_json::to_string(&model_protocols)?,
        model_enabled: serde_json::to_string(&model_enabled)?,
    })
}

fn reconciled_model_enabled(config: &ProviderPreset, existing: Option<&str>) -> anyhow::Result<String> {
    let current: BTreeMap<String, bool> = existing
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let reconciled: BTreeMap<&str, bool> = config
        .models
        .iter()
        .map(|model| {
            (
                model.id.as_str(),
                current.get(&model.id).copied().unwrap_or(model.enabled),
            )
        })
        .collect();
    Ok(serde_json::to_string(&reconciled)?)
}

async fn ensure_provider(
    repo: &dyn IProviderRepository,
    encryption_secret: &str,
    config: &ProviderPreset,
) -> anyhow::Result<()> {
    let payloads = provider_payloads(config)?;
    if let Some(existing) = repo.find_by_id(DEFAULT_USER_ID, &config.id).await? {
        let model_enabled = reconciled_model_enabled(config, existing.model_enabled.as_deref())?;
        repo.update(
            DEFAULT_USER_ID,
            &config.id,
            UpdateProviderParams {
                platform: Some(&config.platform),
                name: Some(&config.name),
                base_url: Some(&config.base_url),
                models: Some(&payloads.models),
                model_protocols: Some(Some(&payloads.model_protocols)),
                model_enabled: Some(Some(&model_enabled)),
                is_full_url: Some(config.is_full_url),
                ..Default::default()
            },
        )
        .await
        .context("update computing platform provider")?;
        return Ok(());
    }

    let encryption_key = derive_encryption_key(encryption_secret);
    let encrypted_empty_key = encrypt_string("", &encryption_key).context("encrypt preset provider empty api key")?;
    repo.create(CreateProviderParams {
        id: Some(&config.id),
        user_id: DEFAULT_USER_ID,
        platform: &config.platform,
        name: &config.name,
        base_url: &config.base_url,
        api_key_encrypted: &encrypted_empty_key,
        models: &payloads.models,
        enabled: config.enabled,
        capabilities: "[]",
        context_limit: None,
        model_protocols: Some(&payloads.model_protocols),
        model_enabled: Some(&payloads.model_enabled),
        model_health: None,
        model_settings: "{}",
        bedrock_config: None,
        is_full_url: config.is_full_url,
    })
    .await
    .context("create computing platform provider")?;
    Ok(())
}

fn merge_existing_headers(
    desired: &BTreeMap<String, String>,
    existing_transport_config: Option<&str>,
) -> BTreeMap<String, String> {
    let mut headers = desired.clone();
    let Some(existing_transport_config) = existing_transport_config else {
        return headers;
    };
    let Ok(existing) = serde_json::from_str::<serde_json::Value>(existing_transport_config) else {
        return headers;
    };
    let Some(existing_headers) = existing.get("headers").and_then(serde_json::Value::as_object) else {
        return headers;
    };
    for (name, value) in existing_headers {
        if let Some(value) = value.as_str() {
            headers.insert(name.clone(), value.to_owned());
        }
    }
    headers
}

fn mcp_payloads(
    config: &McpPreset,
    display_name: &str,
    headers: &BTreeMap<String, String>,
) -> anyhow::Result<(String, String)> {
    let transport_config = serde_json::to_string(&serde_json::json!({
        "url": config.url,
        "headers": headers,
    }))?;
    let mut mcp_servers = BTreeMap::new();
    mcp_servers.insert(
        display_name.to_owned(),
        serde_json::json!({
            "description": config.description,
            "type": "streamable_http",
            "url": config.url,
            "headers": headers,
        }),
    );
    let original_json = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": mcp_servers,
    }))?;
    Ok((transport_config, original_json))
}

async fn managed_mcp_id(
    repo: &dyn IAssistantDefinitionRepository,
    config: &AssistantPreset,
) -> anyhow::Result<Option<String>> {
    let Some(existing) = repo
        .get_global_by_assistant_id_including_deleted(&config.id)
        .await?
    else {
        return Ok(None);
    };
    if existing.source != "builtin" || existing.owner_type != "system" {
        bail!("computing platform assistant must be a builtin system definition");
    }

    // Earlier preset builds already persisted the generated MCP id in this
    // builtin definition. Reuse that id as the stable ownership link instead
    // of treating the user-editable display name as identity.
    if existing.default_model_mode != "fixed"
        || existing.default_model_value.as_deref() != Some(config.default_model.as_str())
        || existing.default_mcps_mode != "fixed"
    {
        return Ok(None);
    }
    let ids: Vec<String> = serde_json::from_str(&existing.default_mcp_ids)
        .context("parse computing platform assistant default MCP ids")?;
    Ok(ids.into_iter().next())
}

async fn available_mcp_name(repo: &dyn IMcpServerRepository, desired: &str) -> anyhow::Result<String> {
    if repo.find_by_name_any(DEFAULT_USER_ID, desired).await?.is_none() {
        return Ok(desired.to_owned());
    }
    for index in 1..=1000 {
        let candidate = if index == 1 {
            format!("{desired} (preset)")
        } else {
            format!("{desired} (preset {index})")
        };
        if repo.find_by_name_any(DEFAULT_USER_ID, &candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    bail!("unable to allocate a unique computing platform MCP name")
}

async fn ensure_mcp(
    repo: &dyn IMcpServerRepository,
    config: &McpPreset,
    managed_id: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(managed_id) = managed_id
        && let Some(existing) = repo.find_by_id_any(DEFAULT_USER_ID, managed_id).await?
    {
        // Deletion is an explicit user choice. Keep the stable reference but
        // never clear deleted_at or create a replacement on startup.
        if existing.deleted_at.is_some() {
            return Ok(existing.id);
        }
        let headers = merge_existing_headers(&config.headers, Some(&existing.transport_config));
        let (transport_config, original_json) = mcp_payloads(config, &existing.name, &headers)?;
        let row = repo
            .update(
                DEFAULT_USER_ID,
                &existing.id,
                UpdateMcpServerParams {
                    // Preserve a user rename; the persisted id is the identity.
                    description: Some(Some(&config.description)),
                    transport_type: Some("http"),
                    transport_config: Some(&transport_config),
                    original_json: Some(Some(&original_json)),
                    builtin: Some(false),
                    ..Default::default()
                },
            )
            .await
            .context("update computing platform MCP")?;
        return Ok(row.id);
    }

    // Never take ownership of a user-created MCP based on name alone.
    let display_name = available_mcp_name(repo, &config.name).await?;
    let (transport_config, original_json) = mcp_payloads(config, &display_name, &config.headers)?;
    let row = repo
        .create(CreateMcpServerParams {
            user_id: DEFAULT_USER_ID,
            name: &display_name,
            description: Some(&config.description),
            enabled: config.enabled,
            transport_type: "http",
            transport_config: &transport_config,
            tools: None,
            original_json: Some(&original_json),
            builtin: false,
        })
        .await
        .context("create computing platform MCP")?;
    Ok(row.id)
}

/// The assistant profile, rules, and skills come from the embedded builtin
/// manifest. This step only applies deployment-specific defaults whose MCP id
/// is generated in the local database.
async fn ensure_assistant_defaults(
    repo: &dyn IAssistantDefinitionRepository,
    config: &AssistantPreset,
    mcp_id: &str,
) -> anyhow::Result<()> {
    let existing = repo
        .get_global_by_assistant_id_including_deleted(&config.id)
        .await?
        .context("computing platform builtin assistant is not materialized")?;
    if existing.source != "builtin" || existing.owner_type != "system" {
        bail!("computing platform assistant must be a builtin system definition");
    }

    let default_mcp_ids = serde_json::to_string(&[mcp_id])?;
    repo.upsert_global(&UpsertAssistantDefinitionParams {
        id: &existing.id,
        assistant_id: &existing.assistant_id,
        source: &existing.source,
        owner_type: &existing.owner_type,
        source_ref: existing.source_ref.as_deref(),
        name: &existing.name,
        name_i18n: &existing.name_i18n,
        description: existing.description.as_deref(),
        description_i18n: &existing.description_i18n,
        avatar_type: &existing.avatar_type,
        avatar_value: existing.avatar_value.as_deref(),
        agent_id: &existing.agent_id,
        rule_resource_type: &existing.rule_resource_type,
        rule_resource_ref: existing.rule_resource_ref.as_deref(),
        recommended_prompts: &existing.recommended_prompts,
        recommended_prompts_i18n: &existing.recommended_prompts_i18n,
        default_model_mode: "fixed",
        default_model_value: Some(&config.default_model),
        default_permission_mode: &existing.default_permission_mode,
        default_permission_value: existing.default_permission_value.as_deref(),
        default_thought_level_mode: &existing.default_thought_level_mode,
        default_thought_level_value: existing.default_thought_level_value.as_deref(),
        default_skills_mode: &existing.default_skills_mode,
        default_skill_ids: &existing.default_skill_ids,
        custom_skill_names: &existing.custom_skill_names,
        default_disabled_builtin_skill_ids: &existing.default_disabled_builtin_skill_ids,
        default_mcps_mode: "fixed",
        default_mcp_ids: &default_mcp_ids,
    })
    .await
    .context("apply computing platform assistant defaults")?;
    Ok(())
}

async fn bootstrap_with_repositories(
    provider_repo: &dyn IProviderRepository,
    mcp_repo: &dyn IMcpServerRepository,
    assistant_repo: &dyn IAssistantDefinitionRepository,
    encryption_secret: &str,
    config: &PresetConfiguration,
) -> anyhow::Result<()> {
    ensure_provider(provider_repo, encryption_secret, &config.provider).await?;
    let previous_mcp_id = managed_mcp_id(assistant_repo, &config.assistant).await?;
    let mcp_id = ensure_mcp(mcp_repo, &config.mcp, previous_mcp_id.as_deref()).await?;
    ensure_assistant_defaults(assistant_repo, &config.assistant, &mcp_id).await?;
    Ok(())
}

/// Best-effort, idempotent bootstrap for the local computing-platform build.
/// Credentials and user enablement choices survive repeated startup passes.
pub(crate) async fn bootstrap_computing_platform_preset(services: &AppServices) -> anyhow::Result<()> {
    let config = load_preset()?;
    let provider_repo = SqliteProviderRepository::new(services.database.pool().clone());
    let mcp_repo = SqliteMcpServerRepository::new(services.database.pool().clone());
    let assistant_repo = SqliteAssistantDefinitionRepository::new(services.database.pool().clone());
    bootstrap_with_repositories(
        &provider_repo,
        &mcp_repo,
        &assistant_repo,
        &services.encryption_secret_raw,
        &config,
    )
    .await?;
    tracing::info!(
        provider_id = %config.provider.id,
        mcp_name = %config.mcp.name,
        assistant_id = %config.assistant.id,
        "computing platform preset is ready"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aionui_db::{init_database_memory, models::Provider};

    const EXPECTED_SKILLS: [&str; 6] = [
        "computing-platform-workloads",
        "computing-platform-tickets",
        "computing-platform-tasks",
        "computing-platform-resources",
        "computing-platform-context",
        "computing-platform-artifacts",
    ];

    async fn seed_builtin_assistant(repo: &dyn IAssistantDefinitionRepository) {
        let skill_ids = serde_json::to_string(&EXPECTED_SKILLS).unwrap();
        repo.upsert_global(&UpsertAssistantDefinitionParams {
            id: "builtin:computing-platform-assistant",
            assistant_id: "computing-platform-assistant",
            source: "builtin",
            owner_type: "system",
            source_ref: Some("computing-platform-assistant"),
            name: "Computing Platform Assistant",
            name_i18n: "{}",
            description: Some("description"),
            description_i18n: "{}",
            avatar_type: "emoji",
            avatar_value: Some("🖥️"),
            agent_id: "aionrs",
            rule_resource_type: "builtin_asset",
            rule_resource_ref: Some("computing-platform-assistant"),
            recommended_prompts: "[]",
            recommended_prompts_i18n: "{}",
            default_model_mode: "auto",
            default_model_value: None,
            default_permission_mode: "auto",
            default_permission_value: None,
            default_thought_level_mode: "auto",
            default_thought_level_value: None,
            default_skills_mode: "fixed",
            default_skill_ids: &skill_ids,
            custom_skill_names: "[]",
            default_disabled_builtin_skill_ids: "[]",
            default_mcps_mode: "auto",
            default_mcp_ids: "[]",
        })
        .await
        .unwrap();
    }

    async fn bootstrap_repos() -> (
        SqliteProviderRepository,
        SqliteMcpServerRepository,
        SqliteAssistantDefinitionRepository,
        PresetConfiguration,
    ) {
        let database = init_database_memory().await.unwrap();
        let provider_repo = SqliteProviderRepository::new(database.pool().clone());
        let mcp_repo = SqliteMcpServerRepository::new(database.pool().clone());
        let assistant_repo = SqliteAssistantDefinitionRepository::new(database.pool().clone());
        seed_builtin_assistant(&assistant_repo).await;
        (provider_repo, mcp_repo, assistant_repo, load_preset().unwrap())
    }

    async fn run_bootstrap(
        provider_repo: &SqliteProviderRepository,
        mcp_repo: &SqliteMcpServerRepository,
        assistant_repo: &SqliteAssistantDefinitionRepository,
        config: &PresetConfiguration,
    ) {
        bootstrap_with_repositories(
            provider_repo,
            mcp_repo,
            assistant_repo,
            "test-encryption-secret",
            config,
        )
        .await
        .unwrap();
    }

    async fn assistant_mcp_id(repo: &SqliteAssistantDefinitionRepository) -> String {
        let assistant = repo
            .get_global_by_assistant_id_including_deleted("computing-platform-assistant")
            .await
            .unwrap()
            .unwrap();
        serde_json::from_str::<Vec<String>>(&assistant.default_mcp_ids)
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn preset_declares_latest_per_model_fields() {
        let config = load_preset().unwrap();
        assert_eq!(config.version, PRESET_VERSION);
        assert_eq!(config.provider.models.len(), 1);
        assert_eq!(config.provider.models[0].id, "qwen3.8-27b");
        assert_eq!(config.provider.models[0].protocol, "openai");
        assert!(config.provider.models[0].enabled);
        assert_eq!(config.assistant.default_model, config.provider.models[0].id);
    }

    #[test]
    fn builtin_assistant_and_skill_corpus_stay_aligned() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../assets/builtin-assistants/assistants.json")).unwrap();
        let assistant = manifest["assistants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|assistant| assistant["id"] == "computing-platform-assistant")
            .unwrap();
        let enabled_skills: Vec<&str> = assistant["enabled_skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|skill| skill.as_str().unwrap())
            .collect();
        assert_eq!(enabled_skills, EXPECTED_SKILLS);
        let corpus = aionui_extension::builtin_skills_corpus();
        for skill in EXPECTED_SKILLS {
            let path = format!("{skill}/SKILL.md");
            assert!(corpus.get_file(&path).is_some(), "missing embedded skill {path}");
        }
    }

    #[test]
    fn preset_rejects_a_disabled_default_model() {
        let invalid = PRESET_JSON.replace("\"enabled\": true", "\"enabled\": false");
        let error = load_preset_from_str(&invalid).unwrap_err();
        assert!(error.to_string().contains("default_model must be enabled"));
    }

    #[tokio::test]
    async fn bootstrap_is_idempotent_and_preserves_credentials_and_toggles() {
        let (provider_repo, mcp_repo, assistant_repo, config) = bootstrap_repos().await;
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let provider = provider_repo
            .find_by_id(DEFAULT_USER_ID, "computing-platform")
            .await
            .unwrap()
            .unwrap();
        assert_provider_shape(&provider);
        let original_api_key = "already-configured-encrypted-key";
        provider_repo
            .update(
                DEFAULT_USER_ID,
                &provider.id,
                UpdateProviderParams {
                    api_key_encrypted: Some(original_api_key),
                    enabled: Some(false),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let mcp_id = assistant_mcp_id(&assistant_repo).await;
        let mcp = mcp_repo.find_by_id(DEFAULT_USER_ID, &mcp_id).await.unwrap().unwrap();
        let configured_transport = serde_json::json!({
            "url": config.mcp.url,
            "headers": {"Authorization": "Bearer configured-token"},
        })
        .to_string();
        mcp_repo
            .update(
                DEFAULT_USER_ID,
                &mcp.id,
                UpdateMcpServerParams {
                    enabled: Some(true),
                    transport_config: Some(&configured_transport),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;

        let provider = provider_repo
            .find_by_id(DEFAULT_USER_ID, "computing-platform")
            .await
            .unwrap()
            .unwrap();
        assert_provider_shape(&provider);
        assert_eq!(provider.api_key_encrypted, original_api_key);
        assert!(!provider.enabled);
        let mcp_after = mcp_repo.find_by_id(DEFAULT_USER_ID, &mcp.id).await.unwrap().unwrap();
        assert_eq!(mcp_after.id, mcp.id);
        assert!(mcp_after.enabled);
        let transport: serde_json::Value = serde_json::from_str(&mcp_after.transport_config).unwrap();
        assert_eq!(transport["headers"]["Authorization"], "Bearer configured-token");
        assert_eq!(assistant_mcp_id(&assistant_repo).await, mcp.id);
    }

    #[tokio::test]
    async fn provider_model_enabled_is_reconciled_without_overwriting_user_choice() {
        let (provider_repo, mcp_repo, assistant_repo, config) = bootstrap_repos().await;
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let disabled_with_stale = serde_json::json!({
            "qwen3.8-27b": false,
            "obsolete-model": true,
        })
        .to_string();
        provider_repo
            .update(
                DEFAULT_USER_ID,
                "computing-platform",
                UpdateProviderParams {
                    model_enabled: Some(Some(&disabled_with_stale)),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let provider = provider_repo
            .find_by_id(DEFAULT_USER_ID, "computing-platform")
            .await
            .unwrap()
            .unwrap();
        let enabled: BTreeMap<String, bool> = serde_json::from_str(provider.model_enabled.as_deref().unwrap()).unwrap();
        assert_eq!(enabled, BTreeMap::from([("qwen3.8-27b".to_owned(), false)]));

        let only_stale = serde_json::json!({"obsolete-model": false}).to_string();
        provider_repo
            .update(
                DEFAULT_USER_ID,
                "computing-platform",
                UpdateProviderParams {
                    model_enabled: Some(Some(&only_stale)),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let provider = provider_repo
            .find_by_id(DEFAULT_USER_ID, "computing-platform")
            .await
            .unwrap()
            .unwrap();
        let enabled: BTreeMap<String, bool> = serde_json::from_str(provider.model_enabled.as_deref().unwrap()).unwrap();
        assert_eq!(enabled, BTreeMap::from([("qwen3.8-27b".to_owned(), true)]));
    }

    #[tokio::test]
    async fn same_name_user_mcp_is_not_claimed_by_the_preset() {
        let (provider_repo, mcp_repo, assistant_repo, config) = bootstrap_repos().await;
        let user_transport = serde_json::json!({
            "url": "http://user.example/mcp",
            "headers": {},
        })
        .to_string();
        let user_mcp = mcp_repo
            .create(CreateMcpServerParams {
                user_id: DEFAULT_USER_ID,
                name: "computing-platform",
                description: Some("user-owned"),
                enabled: true,
                transport_type: "http",
                transport_config: &user_transport,
                tools: None,
                original_json: None,
                builtin: false,
            })
            .await
            .unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;

        let user_after = mcp_repo
            .find_by_id(DEFAULT_USER_ID, &user_mcp.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_after.name, "computing-platform");
        assert_eq!(user_after.description.as_deref(), Some("user-owned"));
        assert_eq!(user_after.transport_config, user_transport);
        let preset_id = assistant_mcp_id(&assistant_repo).await;
        assert_ne!(preset_id, user_mcp.id);
        let preset = mcp_repo.find_by_id(DEFAULT_USER_ID, &preset_id).await.unwrap().unwrap();
        assert_eq!(preset.name, "computing-platform (preset)");
        assert_eq!(mcp_repo.list(DEFAULT_USER_ID).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn renamed_managed_mcp_keeps_its_identity_and_name() {
        let (provider_repo, mcp_repo, assistant_repo, config) = bootstrap_repos().await;
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let mcp_id = assistant_mcp_id(&assistant_repo).await;
        mcp_repo
            .update(
                DEFAULT_USER_ID,
                &mcp_id,
                UpdateMcpServerParams {
                    name: Some("my-computing-platform"),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        assert_eq!(assistant_mcp_id(&assistant_repo).await, mcp_id);
        let row = mcp_repo.find_by_id(DEFAULT_USER_ID, &mcp_id).await.unwrap().unwrap();
        assert_eq!(row.name, "my-computing-platform");
        assert_eq!(mcp_repo.list(DEFAULT_USER_ID).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deleted_managed_mcp_is_not_resurrected() {
        let (provider_repo, mcp_repo, assistant_repo, config) = bootstrap_repos().await;
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;
        let mcp_id = assistant_mcp_id(&assistant_repo).await;
        mcp_repo.delete(DEFAULT_USER_ID, &mcp_id).await.unwrap();
        run_bootstrap(&provider_repo, &mcp_repo, &assistant_repo, &config).await;

        assert!(mcp_repo.find_by_id(DEFAULT_USER_ID, &mcp_id).await.unwrap().is_none());
        let deleted = mcp_repo
            .find_by_id_any(DEFAULT_USER_ID, &mcp_id)
            .await
            .unwrap()
            .unwrap();
        assert!(deleted.deleted_at.is_some());
        assert_eq!(assistant_mcp_id(&assistant_repo).await, mcp_id);
        assert!(mcp_repo.list(DEFAULT_USER_ID).await.unwrap().is_empty());
    }

    fn assert_provider_shape(provider: &Provider) {
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&provider.models).unwrap(),
            vec!["qwen3.8-27b"]
        );
        let protocols: BTreeMap<String, String> =
            serde_json::from_str(provider.model_protocols.as_deref().unwrap()).unwrap();
        assert_eq!(protocols.get("qwen3.8-27b").map(String::as_str), Some("openai"));
    }
}
