use std::collections::HashMap;

use aionui_common::encrypt_string;
use aionui_db::{
    CreateMcpServerParams, CreateProviderParams, IAssistantDefinitionRepository, IMcpServerRepository,
    IProviderRepository, SqliteAssistantDefinitionRepository, SqliteMcpServerRepository, SqliteProviderRepository,
    UpsertAssistantDefinitionParams,
};
use anyhow::{Context, bail};
use serde::Deserialize;

use crate::config::derive_encryption_key;
use crate::services::AppServices;

const DEFAULT_USER_ID: &str = "system_default_user";
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
    models: Vec<String>,
    enabled: bool,
    #[serde(default)]
    is_full_url: bool,
}

#[derive(Debug, Deserialize)]
struct McpPreset {
    name: String,
    description: String,
    url: String,
    enabled: bool,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AssistantPreset {
    id: String,
    default_model: String,
}

fn load_preset() -> anyhow::Result<PresetConfiguration> {
    let config: PresetConfiguration = serde_json::from_str(PRESET_JSON).context("parse computing platform preset")?;
    if config.version == 0 {
        bail!("computing platform preset version must be greater than zero");
    }
    if config.provider.models.is_empty() {
        bail!("computing platform preset requires at least one model");
    }
    if !config
        .provider
        .models
        .iter()
        .any(|model| model == &config.assistant.default_model)
    {
        bail!("assistant.default_model must exist in provider.models");
    }
    Ok(config)
}

async fn ensure_provider(services: &AppServices, config: &PresetConfiguration) -> anyhow::Result<()> {
    let repo = SqliteProviderRepository::new(services.database.pool().clone());
    let existing = repo.list(DEFAULT_USER_ID).await?;
    if existing
        .iter()
        .any(|provider| provider.id == config.provider.id || provider.name == config.provider.name)
    {
        return Ok(());
    }

    let encryption_key = derive_encryption_key(&services.jwt_secret_raw);
    let encrypted_empty_key = encrypt_string("", &encryption_key).context("encrypt preset provider empty api key")?;
    let models = serde_json::to_string(&config.provider.models)?;

    repo.create(CreateProviderParams {
        id: Some(&config.provider.id),
        user_id: DEFAULT_USER_ID,
        platform: &config.provider.platform,
        name: &config.provider.name,
        base_url: &config.provider.base_url,
        api_key_encrypted: &encrypted_empty_key,
        models: &models,
        enabled: config.provider.enabled,
        capabilities: "[]",
        context_limit: None,
        model_protocols: None,
        model_enabled: None,
        model_health: None,
        model_settings: "{}",
        bedrock_config: None,
        is_full_url: config.provider.is_full_url,
    })
    .await?;

    Ok(())
}

async fn ensure_mcp(services: &AppServices, config: &PresetConfiguration) -> anyhow::Result<String> {
    let repo = SqliteMcpServerRepository::new(services.database.pool().clone());
    if let Some(existing) = repo.find_by_name(DEFAULT_USER_ID, &config.mcp.name).await? {
        return Ok(existing.id);
    }

    let transport_config = serde_json::to_string(&serde_json::json!({
        "url": config.mcp.url,
        "headers": config.mcp.headers,
    }))?;

    // AionUi exposes Streamable HTTP as "streamable_http" in MCP JSON, while
    // AionCore uses "http" as the canonical persistence/runtime discriminator.
    let original_server = serde_json::json!({
        "description": config.mcp.description,
        "type": "streamable_http",
        "url": config.mcp.url,
        "headers": config.mcp.headers,
    });
    let mut mcp_servers = serde_json::Map::new();
    mcp_servers.insert(config.mcp.name.clone(), original_server);
    let original_json = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": mcp_servers,
    }))?;

    let row = repo
        .create(CreateMcpServerParams {
            user_id: DEFAULT_USER_ID,
            name: &config.mcp.name,
            description: Some(&config.mcp.description),
            enabled: config.mcp.enabled,
            transport_type: "http",
            transport_config: &transport_config,
            tools: None,
            original_json: Some(&original_json),
            builtin: false,
        })
        .await?;
    Ok(row.id)
}

/// The assistant itself is an official BuiltinAssistant. The builtin manifest
/// owns its profile, rules and six official Skills; this preset only supplies
/// deployment-specific defaults that the generic builtin manifest cannot know:
/// the computing-platform model and the locally-created MCP row id.
async fn ensure_assistant_defaults(
    services: &AppServices,
    config: &PresetConfiguration,
    mcp_id: &str,
) -> anyhow::Result<()> {
    let repo = SqliteAssistantDefinitionRepository::new(services.database.pool().clone());
    let existing = repo
        .get_global_by_assistant_id_including_deleted(&config.assistant.id)
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
        default_model_value: Some(&config.assistant.default_model),
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
    .await?;

    Ok(())
}

/// Best-effort, idempotent computing-platform deployment bootstrap.
///
/// Provider and MCP credentials/toggles are preserved once created. Skills and
/// the assistant profile are official resources embedded in AionCore.
pub(crate) async fn bootstrap_computing_platform_preset(services: &AppServices) -> anyhow::Result<()> {
    let config = load_preset()?;
    ensure_provider(services, &config).await?;
    let mcp_id = ensure_mcp(services, &config).await?;
    ensure_assistant_defaults(services, &config, &mcp_id).await?;
    tracing::info!("computing platform preset is ready");
    Ok(())
}
