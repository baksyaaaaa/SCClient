mod launcher;

pub mod data_dir;

pub use launcher::{LaunchBehaviour, LauncherSettings, ViewLayout, ViewState};
pub use csclient_cluster::{
    GameSettingsProfile, PackageUpdateMode, ProfileUpdate, SettingsOsExtra,
};
pub use csclient_common::Resolution;

pub mod store;

/// Pushed down by the composition layer on every settings change so the HTTP
/// client never reaches back into global settings (and thus into its own owner)
#[must_use]
pub fn net_config(settings: &LauncherSettings) -> csclient_net::NetConfig {
    csclient_net::NetConfig {
        nightly_bundles: settings.nightly_bundles,
        ..csclient_net::NetConfig::default().with_overrides(
            settings.curseforge_api_key.as_deref(),
            settings.modrinth_api_key.as_deref(),
            settings.custom_api_endpoint.as_deref(),
            settings.custom_meta_url_base.as_deref(),
            settings.custom_plus_endpoint.as_deref(),
        )
    }
}
