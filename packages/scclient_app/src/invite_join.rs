use scclient_common::GameLoader;
use scclient_core::LinkedArtifactInfo;
use scclient_core::versions::VersionMetadata;

use crate::chat::InviteGame;
use crate::essential::SCPLUS;

#[must_use]
pub fn join_jvm_arg(invite_id: i32) -> String {
    format!("-Dcsplus.joinInvite={invite_id}")
}

#[derive(Clone, Debug, PartialEq)]
pub enum InviteJoin {
    Launch(i64),
    Pick(Vec<i64>),
    Create {
        version: String,
        loader: GameLoader,
        scclient: bool,
        unlinked: bool,
    },
    Unknown,
}

#[must_use]
pub fn plan(
    game: &InviteGame,
    candidates: Vec<i64>,
    unlinked: bool,
    scclient: bool,
) -> InviteJoin {
    let Some((version, loader)) = game.known() else {
        return if candidates.is_empty() {
            InviteJoin::Unknown
        } else {
            InviteJoin::Pick(candidates)
        };
    };

    match candidates.as_slice() {
        [] => InviteJoin::Create {
            version: version.to_string(),
            loader,
            scclient,
            unlinked,
        },
        [only] => InviteJoin::Launch(*only),
        _ => InviteJoin::Pick(candidates),
    }
}

#[must_use]
pub fn cluster_matches(game: &InviteGame, version: &str, loader: GameLoader) -> bool {
    game.known()
        .is_none_or(|(wanted, wanted_loader)| wanted == version && wanted_loader == loader)
}

#[must_use]
pub fn wizard_invite(
    invite: Option<i32>,
    prefill: Option<&(String, GameLoader)>,
    version: Option<&str>,
    loader: Option<GameLoader>,
) -> Option<i32> {
    invite.filter(|_| {
        prefill.is_some_and(|(wanted, wanted_loader)| {
            version == Some(wanted.as_str()) && loader == Some(*wanted_loader)
        })
    })
}

#[must_use]
pub fn custom_loader(loader: GameLoader) -> bool {
    loader != GameLoader::Quilt
}

#[must_use]
pub fn has_scplus(artifacts: &[LinkedArtifactInfo], local: &[String]) -> bool {
    artifacts
        .iter()
        .any(|artifact| artifact.enabled && is_scplus(artifact, local))
}

#[must_use]
pub fn disabled_scplus<'a>(
    artifacts: &'a [LinkedArtifactInfo],
    local: &[String],
) -> Option<&'a str> {
    artifacts
        .iter()
        .find(|artifact| !artifact.enabled && is_scplus(artifact, local))
        .map(|artifact| artifact.hash.as_str())
}

pub const SCPLUS_MOD_ID: &str = "polyplus";

#[must_use]
pub fn maybe_local(artifact: &LinkedArtifactInfo) -> bool {
    artifact.content_type == scclient_common::ContentType::Mod
        && matches!(
            artifact.provider,
            None | Some(scclient_common::ProviderId::Local)
        )
}

fn is_scplus(artifact: &LinkedArtifactInfo, local: &[String]) -> bool {
    (artifact.provider == Some(SCPLUS.provider)
        && artifact.project_id.as_deref() == Some(SCPLUS.project_id))
        || local.contains(&artifact.hash)
}

#[must_use]
pub fn scclient_supports(metadata: &[VersionMetadata], version: &str, loader: GameLoader) -> bool {
    metadata.iter().any(|entry| {
        entry.mc_version().as_deref() == Some(version)
            && entry
                .loader
                .as_deref()
                .and_then(|name| name.parse::<GameLoader>().ok())
                == Some(loader)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> InviteGame {
        InviteGame {
            version: Some("1.21.1".to_string()),
            loader: Some(GameLoader::Fabric),
        }
    }

    fn entry(minor: u32, patch: Option<u32>, loader: &str) -> VersionMetadata {
        VersionMetadata {
            major_version: 21,
            minor_version: Some(minor),
            patch_version: patch,
            loader: Some(loader.to_string()),
            name: String::new(),
            art_url: None,
            long_description: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn one_match_launches_and_several_ask() {
        assert_eq!(plan(&known(), vec![4], false, true), InviteJoin::Launch(4));
        assert_eq!(plan(&known(), vec![4, 5], false, true), InviteJoin::Pick(vec![4, 5]));
    }

    #[test]
    fn no_match_offers_to_create_and_remembers_support() {
        assert_eq!(
            plan(&known(), Vec::new(), false, true),
            InviteJoin::Create {
                version: "1.21.1".to_string(),
                loader: GameLoader::Fabric,
                scclient: true,
                unlinked: false,
            }
        );
    }

    #[test]
    fn an_unknown_game_never_launches_on_its_own() {
        let unknown = InviteGame::default();

        assert_eq!(plan(&unknown, vec![4], false, true), InviteJoin::Pick(vec![4]));
        assert_eq!(plan(&unknown, Vec::new(), false, true), InviteJoin::Unknown);
    }

    #[test]
    fn half_known_games_count_as_unknown() {
        let game = InviteGame {
            version: Some("1.21.1".to_string()),
            loader: None,
        };

        assert_eq!(plan(&game, vec![4], false, true), InviteJoin::Pick(vec![4]));
    }

    #[test]
    fn a_matching_cluster_without_scplus_is_offered_for_reuse() {
        assert!(matches!(
            plan(&known(), Vec::new(), true, true),
            InviteJoin::Create { unlinked: true, .. }
        ));
    }

    #[test]
    fn the_wizard_only_joins_when_it_still_makes_the_invited_game() {
        let prefill = ("1.21.1".to_string(), GameLoader::Fabric);
        let wanted = Some(&prefill);

        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.21.1"), Some(GameLoader::Fabric)),
            Some(3)
        );
        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.20.1"), Some(GameLoader::Fabric)),
            None
        );
        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.21.1"), Some(GameLoader::Forge)),
            None
        );
        assert_eq!(
            wizard_invite(None, wanted, Some("1.21.1"), Some(GameLoader::Fabric)),
            None
        );
    }

    #[test]
    fn a_custom_cluster_is_offered_for_every_loader_the_wizard_keeps() {
        assert!(custom_loader(GameLoader::Ornithe));
        assert!(custom_loader(GameLoader::Fabric));
        assert!(!custom_loader(GameLoader::Quilt));
    }

    #[test]
    fn matching_needs_the_exact_version_and_loader() {
        assert!(cluster_matches(&known(), "1.21.1", GameLoader::Fabric));
        assert!(!cluster_matches(&known(), "1.21", GameLoader::Fabric));
        assert!(!cluster_matches(&known(), "1.21.1", GameLoader::Quilt));
        assert!(cluster_matches(
            &InviteGame::default(),
            "1.8.9",
            GameLoader::Forge
        ));
    }

    #[test]
    fn support_needs_the_exact_row_in_the_catalogue() {
        let catalogue = [entry(1, None, "fabric")];

        assert!(scclient_supports(&catalogue, "1.21.1", GameLoader::Fabric));
        assert!(!scclient_supports(&catalogue, "1.21.1", GameLoader::Forge));
        assert!(!scclient_supports(&catalogue, "1.21.2", GameLoader::Fabric));
    }

    fn artifact(hash: &str, enabled: bool, project: &str) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: hash.to_string(),
            cluster_file_name: String::new(),
            enabled,
            content_type: scclient_common::ContentType::Mod,
            file_name: String::new(),
            size_bytes: None,
            project_id: Some(project.to_string()),
            version_id: None,
            display_name: None,
            display_version: None,
            provider: Some(SCPLUS.provider),
            published_at: None,
            added_at: None,
            seen_status: scclient_core::SeenStatus::Seen,
        }
    }

    #[test]
    fn a_disabled_scplus_is_enabled_instead_of_installed_again() {
        let disabled = [artifact("other", false, "else"), artifact("plus", false, SCPLUS.project_id)];
        assert!(!has_scplus(&disabled, &[]));
        assert_eq!(disabled_scplus(&disabled, &[]), Some("plus"));

        let enabled = [artifact("plus", true, SCPLUS.project_id)];
        assert!(has_scplus(&enabled, &[]));
        assert_eq!(disabled_scplus(&enabled, &[]), None);

        assert_eq!(disabled_scplus(&[artifact("other", false, "else")], &[]), None);
    }

    #[test]
    fn a_local_scplus_counts_too() {
        let mut local = artifact("dev", true, "dev");
        local.provider = Some(scclient_common::ProviderId::Local);
        let hashes = ["dev".to_string()];

        assert!(maybe_local(&local));
        assert!(!has_scplus(std::slice::from_ref(&local), &[]));
        assert!(has_scplus(std::slice::from_ref(&local), &hashes));

        local.enabled = false;
        assert_eq!(
            disabled_scplus(std::slice::from_ref(&local), &hashes),
            Some("dev")
        );
    }

    #[test]
    fn the_join_argument_carries_the_invite() {
        assert_eq!(join_jvm_arg(38), "-Dcsplus.joinInvite=38");
    }
}
