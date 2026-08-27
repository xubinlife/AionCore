#![warn(clippy::disallowed_types)]

//! Application crate: assembles all domain crates into an Axum server with DI and middleware.
//!
//! This file is a public façade — it only re-exports symbols defined in
//! submodules. All logic lives in the modules below.

mod config;
mod preset;
mod router;
mod services;

pub use config::{AppConfig, IdentityMode, derive_encryption_key};
pub use router::{
    ChannelOrchestratorComponents, ModuleStates, RouterBuildError, RouterRuntime, build_assistant_state,
    build_conversation_state, build_extension_states, build_module_states, build_ws_state, create_router,
    create_router_with_all_state, create_router_with_states,
};
pub use services::AppServices;

pub async fn create_router_with_runtime(
    services: &AppServices,
) -> Result<(axum::Router, RouterRuntime), RouterBuildError> {
    let result = router::create_router_with_runtime(services).await?;

    if services.identity_mode.is_local()
        && let Err(error) = preset::bootstrap_computing_platform_preset(services).await
    {
        tracing::warn!(error = %error, "computing platform preset bootstrap failed; continuing startup");
    }

    Ok(result)
}
