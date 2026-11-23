use freya::query::{Mutation, MutationCapability, QueriesStorage, UseMutation, use_mutation};
use scclient_common::domain::GameLoader;
use scclient_content::packages::LiveSync;
use scclient_db::models::{ClusterId, ClusterKind};
use std::path::PathBuf;

use super::bundles::{
    BundleOverridesQuery, BundleUpdatesQuery, BundlesWithStatusQuery, ImportedModpacksQuery,
    ModpackSourcesQuery, UNCREATED_CLUSTER, bundle_plan,
};
use super::cluster_content::{
    ClusterContentQuery, FolderPacksQuery, MigratableRoutesQuery, ShadowedModsQuery,
};
use super::clusters::{ListClustersQuery, SharedGroupsQuery};
use super::package_updates::PackageUpdatesQuery;
use super::settings_profiles::{
    ClusterProfileQuery, ClusterSettingsQuery, GameProfileQuery, ListNamedProfilesQuery,
};
use crate::i18n::{t, tf};

async fn timed(step: &'static str, fut: impl std::future::Future<Output = ()>) {
    let started = std::time::Instant::now();
    fut.await;
    tracing::debug!(
        target: "scclient_app::perf",
        step,
        ms = started.elapsed().as_millis() as u64,
        "invalidate step"
    );
}

pub async fn invalidate_cluster_queries() {
    let started = std::time::Instant::now();
    timed(
        "cluster_content",
        QueriesStorage::<ClusterContentQuery>::invalidate_all(),
    )
    .await;
    timed(
        "folder_packs",
        QueriesStorage::<FolderPacksQuery>::invalidate_all(),
    )
    .await;
    timed(
        "shadowed_mods",
        QueriesStorage::<ShadowedModsQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_overrides",
        QueriesStorage::<BundleOverridesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundles_with_status",
        QueriesStorage::<BundlesWithStatusQuery>::invalidate_all(),
    )
    .await;
    timed(
        "migratable_routes",
        QueriesStorage::<MigratableRoutesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "shared_groups",
        QueriesStorage::<SharedGroupsQuery>::invalidate_all(),
    )
    .await;
    timed(
        "clusters",
        QueriesStorage::<ListClustersQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_updates",
        QueriesStorage::<BundleUpdatesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "package_updates",
        QueriesStorage::<PackageUpdatesQuery>::invalidate_all(),
    )
    .await;
    timed("bisect", super::bisect::invalidate_bisect_queries()).await;
    timed(
        "modpack_sources",
        QueriesStorage::<ModpackSourcesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "imported_modpacks",
        QueriesStorage::<ImportedModpacksQuery>::invalidate_all(),
    )
    .await;
    tracing::debug!(
        target: "scclient_app::perf",
        ms = started.elapsed().as_millis() as u64,
        "cluster queries invalidated"
    );
}

/// Split out of [`invalidate_cluster_queries`] so an install can wait for just
/// this before dropping its busy flag
pub async fn invalidate_cluster_content_queries() {
    crate::hooks::cloud_sync_signal(crate::hooks::CloudSignal::Changed(None));
    QueriesStorage::<ClusterContentQuery>::invalidate_all().await;
    QueriesStorage::<MigratableRoutesQuery>::invalidate_all().await;
}

/// Everything [`invalidate_cluster_queries`] does bar the cluster list and the
/// cached package updates neither of which an enabled flag can move
/// The bundle queries do move a toggle writes a bundle override and both read
/// those back
async fn invalidate_enabled_flag_queries() {
    timed(
        "cluster_content",
        QueriesStorage::<ClusterContentQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_overrides",
        QueriesStorage::<BundleOverridesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundles_with_status",
        QueriesStorage::<BundlesWithStatusQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_updates",
        QueriesStorage::<BundleUpdatesQuery>::invalidate_all(),
    )
    .await;
}

pub async fn invalidate_profile_queries() {
    QueriesStorage::<ListNamedProfilesQuery>::invalidate_all().await;
    QueriesStorage::<GameProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterSettingsQuery>::invalidate_all().await;
    QueriesStorage::<ListClustersQuery>::invalidate_all().await;
}

pub(crate) async fn take_bundles(
    actions: &crate::hooks::Actions,
    state: &std::sync::Arc<scclient_core::LauncherState>,
    cluster_id: ClusterId,
    name: &str,
    bundles: &[String],
) {
    if bundles.is_empty() {
        return;
    }
    let state = state.clone();
    let bundles = bundles.to_vec();
    let failure = crate::launcher::off_ui(async move {
        match state.clusters.set_takes_bundles(cluster_id).await {
            Ok(cluster) => {
                let failure = set_up_bundles(&state, &cluster, Some(&bundles)).await;
                scclient_content::modpacks::recheck_duplicates(
                    cluster_id,
                    scclient_content::modpacks::MODPACK_BUNDLE_NAME,
                    &state.services.content(),
                )
                .await;
                failure
            }
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not turn on bundles for the improved instance");
                Some(err.to_string())
            }
        }
    })
    .await;
    if let Some(reason) = failure {
        actions
            .notify(tf("scclient.mutations.take_bundles_failed", &[&name]))
            .body(reason)
            .error()
            .send();
    }
}

async fn set_up_bundles(
    state: &std::sync::Arc<scclient_core::LauncherState>,
    cluster: &scclient_core::clusters::Cluster,
    selected: Option<&[String]>,
) -> Option<String> {
    let content = &state.services.content();
    if let Some(selected) = selected {
        let archives = state
            .bundles
            .archives_for(content, &cluster.mc_version, cluster.mc_loader)
            .await
            .unwrap_or_default();
        let (overrides, choices) = bundle_plan(
            UNCREATED_CLUSTER,
            &archives,
            &selected.iter().cloned().collect(),
        );
        if let Err(err) =
            scclient_core::set_bundle_package_overrides(cluster.id, &overrides, content).await
        {
            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the bundle choices for the new instance");
        }
        if let Err(err) =
            scclient_content::bundles::set_bundle_choices(cluster.id, &choices, content).await
        {
            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the bundle choices for the new instance");
        }
    }

    let session = scclient_events::GroupedProgressSession::start(
        &state.services.events,
        tf("scclient.mutations.setting_up", &[&cluster.name]),
    );
    let mut failure = None;
    if let Err(err) = scclient_content::bundles::install_cluster_bundles(
        cluster.id,
        state.bundles.as_ref(),
        Some(&session),
        content,
        None,
    )
    .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to install bundle content for the new instance");
        failure = Some(err.to_string());
    }
    if let Err(err) =
        scclient_core::apply_bundle_java_override(state, cluster.id, true, true, Some(&session))
            .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to apply the bundle java override for the new instance");
    }
    session.finish();
    failure
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ClusterMutation;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ClusterAction {
    SetArtifactEnabled {
        cluster_id: ClusterId,
        hash: String,
        enabled: bool,
    },
    RemoveArtifact {
        cluster_id: ClusterId,
        hash: String,
    },
    RemoveBundlePackageFromDisk {
        cluster_id: ClusterId,
        hash: String,
    },
    SetFolderPackEnabled {
        cluster_id: ClusterId,
        name: String,
        enabled: bool,
    },
    RemoveFolderPack {
        cluster_id: ClusterId,
        name: String,
        enabled: bool,
    },
    DeleteBundledArtifact {
        cluster_id: ClusterId,
        hash: String,
    },
    SetBundlePackageEnabled {
        cluster_id: ClusterId,
        bundle_name: String,
        package_id: String,
        enabled: bool,
        /// Manifest default matching it clears the override contradicting it
        /// writes `Enabled` / `Disabled`
        manifest_default: bool,
    },
    SetBundlePackageRemoved {
        cluster_id: ClusterId,
        bundle_name: String,
        package_id: String,
        hash: Option<String>,
        removed: bool,
    },
    SetDirTarget {
        cluster_id: ClusterId,
        target: scclient_core::clusters::DirTarget,
        create: bool,
    },
    DeleteSharedGroup {
        name: String,
    },
    VerifyFiles {
        cluster_id: ClusterId,
    },
    CreateInstance {
        kind: ClusterKind,
        name: String,
        mc_version: String,
        mc_loader: GameLoader,
        mc_loader_version: Option<String>,
        description: Option<String>,
        tags: Vec<String>,
        cover_source: Option<PathBuf>,
        bundles: Option<Vec<String>>,
        require_bundles: bool,
        dedicated: bool,
        shared_group: Option<String>,
    },
    UpdateInstance {
        cluster_id: ClusterId,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
        cover_source: Option<PathBuf>,
        clear_cover: bool,
    },
    DeleteInstance {
        cluster_id: ClusterId,
    },
    ResetInstance {
        cluster_id: ClusterId,
    },
    Batch(Vec<ClusterAction>),
}

impl MutationCapability for ClusterMutation {
    type Ok = Option<ClusterId>;
    type Err = String;
    type Keys = ClusterAction;

    async fn run(&self, keys: &ClusterAction) -> Result<Option<ClusterId>, String> {
        if let ClusterAction::Batch(actions) = keys {
            let mut errors = Vec::new();
            for action in actions {
                if let Err(err) = Box::pin(self.run(action)).await {
                    errors.push(err);
                }
            }
            return if errors.is_empty() {
                Ok(None)
            } else {
                Err(errors.join("\n"))
            };
        }
        let started = std::time::Instant::now();
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        let services = &state.services;
        let content = &state.services.content();
        let notify_deferred = |live: LiveSync, cluster_id: ClusterId| {
            if live == LiveSync::Deferred && state.games.is_active(cluster_id) {
                services
                    .events
                    .notify(t("scclient.mutations.deferred.title"))
                    .body(t("scclient.mutations.deferred.body"))
                    .send();
            }
        };
        let mut created = None;
        let result = match keys {
            ClusterAction::SetArtifactEnabled { cluster_id, .. }
            | ClusterAction::RemoveArtifact { cluster_id, .. }
            | ClusterAction::RemoveBundlePackageFromDisk { cluster_id, .. }
            | ClusterAction::DeleteBundledArtifact { cluster_id, .. }
            | ClusterAction::SetBundlePackageEnabled { cluster_id, .. }
            | ClusterAction::SetBundlePackageRemoved { cluster_id, .. }
            | ClusterAction::SetDirTarget { cluster_id, .. }
            | ClusterAction::VerifyFiles { cluster_id }
            | ClusterAction::DeleteInstance { cluster_id }
            | ClusterAction::ResetInstance { cluster_id }
                if crate::hooks::update_check_running(*cluster_id) =>
            {
                Err(scclient_content::ContentError::InvalidData {
                    reason: t("scclient.mutations.blocked.update_check").to_string(),
                })
            }
            ClusterAction::SetArtifactEnabled {
                cluster_id,
                hash,
                enabled,
            } => scclient_core::set_artifact_enabled_to(*cluster_id, hash, *enabled, content)
                .await
                .map(|live| notify_deferred(live, *cluster_id)),
            ClusterAction::RemoveArtifact { cluster_id, hash } => {
                scclient_core::delete_artifact(*cluster_id, hash, content).await
            }
            ClusterAction::SetFolderPackEnabled {
                cluster_id,
                name,
                enabled,
            } => {
                folder_pack_op(&state, *cluster_id, |dir| async move {
                    scclient_core::game::set_folder_pack_enabled(&dir, name, *enabled).await
                })
                .await
            }
            ClusterAction::RemoveFolderPack {
                cluster_id,
                name,
                enabled,
            } => {
                folder_pack_op(&state, *cluster_id, |dir| async move {
                    scclient_core::game::remove_folder_pack(&dir, name, *enabled).await
                })
                .await
            }
            ClusterAction::RemoveBundlePackageFromDisk { cluster_id, hash } => {
                scclient_core::remove_artifact_from_cluster(*cluster_id, hash, false, content)
                    .await
            }
            ClusterAction::DeleteBundledArtifact { cluster_id, hash } => {
                async {
                    let live =
                        scclient_core::set_artifact_enabled_to(*cluster_id, hash, false, content)
                            .await?;
                    notify_deferred(live, *cluster_id);
                    scclient_core::on_user_remove_artifact(*cluster_id, hash, content).await
                }
                .await
            }
            ClusterAction::SetBundlePackageEnabled {
                cluster_id,
                bundle_name,
                package_id,
                enabled,
                manifest_default,
            } => {
                scclient_core::set_bundle_package_enabled(
                    *cluster_id,
                    bundle_name,
                    package_id,
                    *enabled,
                    *manifest_default,
                    content,
                )
                .await
            }
            ClusterAction::SetBundlePackageRemoved {
                cluster_id,
                bundle_name,
                package_id,
                hash,
                removed,
            } => {
                async {
                    let live = scclient_core::set_bundle_package_removed(
                        *cluster_id,
                        bundle_name,
                        package_id,
                        hash.as_deref(),
                        *removed,
                        content,
                    )
                    .await?;
                    notify_deferred(live, *cluster_id);
                    // A deleted mod has no files left, so a restore downloads it again now
                    if !*removed {
                        scclient_content::bundles::install_cluster_bundles(
                            *cluster_id,
                            state.bundles.as_ref(),
                            None,
                            content,
                            None,
                        )
                        .await?;
                    }
                    Ok(())
                }
                .await
            }
            ClusterAction::SetDirTarget { cluster_id, .. }
                if state.games.is_syncing(*cluster_id) =>
            {
                Err(scclient_content::ContentError::InvalidData {
                    reason: t("scclient.mutations.blocked.sync_before_folder").to_string(),
                })
            }
            ClusterAction::SetDirTarget {
                cluster_id,
                target,
                create,
            } => {
                // Switching moves the instance's files, which a reset is moving too
                if scclient_core::clusters::is_resetting(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.reset_before_folder").to_string(),
                    })
                } else if crate::hooks::modpack_job_running(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.modpack_folder").to_string(),
                    })
                } else if scclient_core::clusters::has_imported_modpacks(&state, *cluster_id).await
                {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.modpacks_before_folder").to_string(),
                    })
                } else {
                    use scclient_core::clusters::{ClusterError, DirTarget};
                    let previous = state
                        .clusters
                        .get(*cluster_id)
                        .await
                        .ok()
                        .map(|cluster| cluster.dir_target);
                    let mut target = target.clone();
                    let mut result = Ok(());
                    if *create && state.games.is_active(*cluster_id) {
                        result = Err(ClusterError::AlreadyRunning(*cluster_id).to_string());
                    } else if let DirTarget::Shared(Some(name)) = &target {
                        let resolved = if *create {
                            state.clusters.create_shared_group(name).await.map(Some)
                        } else {
                            state.clusters.find_shared_group(name).await
                        };
                        match resolved {
                            Ok(Some(group)) => target = DirTarget::Shared(Some(group)),
                            Ok(None) => {
                                result =
                                    Err(ClusterError::SharedGroupMissing(name.clone()).to_string())
                            }
                            Err(err) => result = Err(err.to_string()),
                        }
                    }
                    let switching = previous.as_ref().is_some_and(|from| *from != target);
                    if result.is_ok() {
                        result = state
                            .clusters
                            .set_dir_target(
                                *cluster_id,
                                &target,
                                state.games.is_active(*cluster_id),
                            )
                            .await
                            .map_err(|err| err.to_string());
                    }
                    if result.is_ok()
                        && switching
                        && let Ok(cluster) = state.clusters.get(*cluster_id).await
                    {
                        if let Some(DirTarget::Shared(from)) = &previous
                            && let Ok(dir) = cluster.dir()
                            && let Err(err) = scclient_core::game::leave_shared_folder(
                                &state.services,
                                &cluster,
                                &dir,
                                from.as_deref(),
                            )
                            .await
                        {
                            if let Some(previous) = &previous {
                                state
                                    .clusters
                                    .set_dir_target(*cluster_id, previous, false)
                                    .await
                                    .ok();
                            }
                            result = Err(err.to_string());
                        } else if matches!(target, DirTarget::Shared(_)) {
                            scclient_core::game::join_shared_folder(&state.services, &cluster)
                                .await;
                        }
                    }
                    result.map_err(|reason| scclient_content::ContentError::InvalidData { reason })
                }
            }
            ClusterAction::DeleteSharedGroup { .. } if state.games.any_syncing() => {
                Err(scclient_content::ContentError::InvalidData {
                    reason: t("scclient.mutations.blocked.sync_delete_folder").to_string(),
                })
            }
            ClusterAction::DeleteSharedGroup { name } => state
                .clusters
                .delete_shared_group(name)
                .await
                .map_err(|err| scclient_content::ContentError::InvalidData {
                    reason: err.to_string(),
                }),
            ClusterAction::VerifyFiles { cluster_id } => {
                // Reports its own outcome not the generic failure toast a
                // verify that finds nothing wrong is still a useful result
                let (state, cluster_id) = (state.clone(), *cluster_id);
                match crate::launcher::off_ui(async move {
                    scclient_core::verify_cluster_files(&state, cluster_id).await
                })
                .await
                {
                    Ok(report) => {
                        let notify = services
                            .events
                            .notify(t("scclient.mutations.verification_complete"));
                        let notify = notify.body(report.summary());
                        if report.unrepairable.is_empty() {
                            notify.send();
                        } else {
                            notify.error().send();
                        }
                        Ok(())
                    }
                    Err(err) => Err(scclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    }),
                }
            }
            ClusterAction::CreateInstance {
                kind,
                name,
                mc_version,
                mc_loader,
                mc_loader_version,
                description,
                tags,
                cover_source,
                bundles,
                require_bundles,
                dedicated,
                shared_group,
            } => {
                let global = state.settings.read().global_game_settings.clone();
                let mut options = scclient_core::clusters::CreateClusterOptions::new(
                    name.clone(),
                    mc_version.clone(),
                    *mc_loader,
                )
                .kind(*kind)
                .user_created(true)
                .tags(tags.clone());
                options.mc_loader_version = mc_loader_version.clone();
                options.description = description.clone();

                match state.clusters.create(&global, options).await {
                    Ok(cluster) => {
                        if *dedicated
                            && let Err(err) = state
                                .clusters
                                .set_dedicated_dir(cluster.id, true, false)
                                .await
                        {
                            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to give the new instance a dedicated directory");
                            services
                                .events
                                .notify(t("scclient.mutations.dedicated_dir_failed.title"))
                                .body(tf("scclient.mutations.dedicated_dir_failed.body", &[&err]))
                                .error()
                                .send();
                        }
                        if let Some(group) = shared_group
                            && let Err(err) = state
                                .clusters
                                .set_dir_target(
                                    cluster.id,
                                    &scclient_core::clusters::DirTarget::Shared(Some(
                                        group.clone(),
                                    )),
                                    false,
                                )
                                .await
                        {
                            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to put the new instance in its shared folder");
                            services
                                .events
                                .notify(tf(
                                    "scclient.mutations.shared_group_failed.title",
                                    &[group],
                                ))
                                .body(tf("scclient.mutations.shared_group_failed.body", &[&err]))
                                .error()
                                .send();
                        }

                        services
                            .events
                            .signal(scclient_events::Signal::ClustersChanged);

                        if let Some(source) = cover_source
                            && let Err(err) =
                                state.clusters.set_cover_from_file(cluster.id, source).await
                        {
                            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to store the instance cover");
                        }

                        let bundle_failure = if cluster.uses_bundles() {
                            set_up_bundles(&state, &cluster, bundles.as_deref()).await
                        } else {
                            None
                        };
                        match bundle_failure.filter(|_| *require_bundles) {
                            Some(reason) => {
                                match state.clusters.delete(cluster.id, true).await {
                                    Ok(_) => {
                                        crate::hooks::forget_deleted_cluster(&state, cluster.id)
                                            .await;
                                        services
                                            .events
                                            .signal(scclient_events::Signal::ClustersChanged);
                                    }
                                    Err(err) => {
                                        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to remove the instance whose bundles did not install");
                                    }
                                }
                                Err(scclient_content::ContentError::InvalidData { reason })
                            }
                            None => {
                                created = Some(cluster.id);
                                Ok(())
                            }
                        }
                    }
                    Err(err) => Err(scclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    }),
                }
            }
            ClusterAction::UpdateInstance {
                cluster_id,
                name,
                description,
                tags,
                cover_source,
                clear_cover,
            } => {
                let cover = if *clear_cover {
                    state.clusters.clear_cover(*cluster_id).await.ok();
                    scclient_common::Patch::Clear
                } else if let Some(source) = cover_source {
                    match state
                        .clusters
                        .set_cover_from_file(*cluster_id, source)
                        .await
                    {
                        Ok(_) => scclient_common::Patch::Unchanged,
                        Err(err) => {
                            tracing::warn!(cluster_id, error = %err, "failed to store the instance cover");
                            scclient_common::Patch::Unchanged
                        }
                    }
                } else {
                    scclient_common::Patch::Unchanged
                };

                let update = scclient_core::clusters::ClusterUpdate {
                    name: Some(name.clone()),
                    description: match description {
                        Some(text) => scclient_common::Patch::Set(text.clone()),
                        None => scclient_common::Patch::Clear,
                    },
                    tags: Some(tags.clone()),
                    cover_path: cover,
                    ..Default::default()
                };

                state
                    .clusters
                    .update(*cluster_id, update)
                    .await
                    .map(|_| ())
                    .map_err(|err| scclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    })
            }
            ClusterAction::DeleteInstance { cluster_id } => {
                if crate::hooks::modpack_job_running(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.modpack_before_delete").to_string(),
                    })
                } else if state.games.is_active(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.game_before_delete").to_string(),
                    })
                } else if state.games.is_syncing(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.sync_before_delete").to_string(),
                    })
                } else {
                    let (released, id) = (state.clone(), *cluster_id);
                    crate::launcher::off_ui(async move {
                        scclient_core::clusters::release_imported_modpacks(&released, id).await;
                    })
                    .await;
                    let outcome = state
                        .clusters
                        .delete(*cluster_id, true)
                        .await
                        .map_err(|err| scclient_content::ContentError::InvalidData {
                            reason: err.to_string(),
                        });
                    if outcome.is_ok() {
                        crate::hooks::forget_deleted_cluster(&state, *cluster_id).await;
                        services
                            .events
                            .signal(scclient_events::Signal::ClustersChanged);
                    }
                    outcome
                }
            }
            ClusterAction::ResetInstance { cluster_id } => {
                // Claimed here on the UI thread, where modpack jobs are claimed too, so neither
                // can slip in while the other is being handed to a background task
                let claim = scclient_core::clusters::ResetClaim::take(*cluster_id);
                if crate::hooks::modpack_job_running(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.modpack_before_reset").to_string(),
                    })
                } else if crate::hooks::exporting(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.export_before_reset").to_string(),
                    })
                } else if state.games.is_active(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.game_before_reset").to_string(),
                    })
                } else if state.games.is_syncing(*cluster_id) {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.sync_before_reset").to_string(),
                    })
                } else if let Some(claim) = claim {
                    let (state, cluster_id) = (state.clone(), *cluster_id);
                    let outcome = crate::launcher::off_ui(async move {
                        let name = state
                            .clusters
                            .get(cluster_id)
                            .await
                            .map(|cluster| cluster.name)
                            .unwrap_or_default();
                        let session = scclient_events::GroupedProgressSession::start(
                            &state.services.events,
                            tf("scclient.mutations.resetting", &[&name]),
                        );
                        let result =
                            scclient_core::clusters::reset_cluster(&state, claim, Some(&session))
                                .await;
                        session.finish();
                        result
                    })
                    .await;
                    services
                        .events
                        .signal(scclient_events::Signal::ClustersChanged);
                    invalidate_profile_queries().await;
                    invalidate_cluster_queries().await;
                    match outcome {
                        Ok(report) => {
                            notify_reset(services, &report);
                            Ok(())
                        }
                        Err(err) => Err(scclient_content::ContentError::InvalidData {
                            reason: err.to_string(),
                        }),
                    }
                } else {
                    Err(scclient_content::ContentError::InvalidData {
                        reason: t("scclient.mutations.blocked.already_resetting").to_string(),
                    })
                }
            }
            ClusterAction::Batch(_) => unreachable!(),
        };
        tracing::debug!(
            target: "scclient_app::perf",
            ms = started.elapsed().as_millis() as u64,
            ok = result.is_ok(),
            "cluster action ran"
        );
        result.map(|()| created).map_err(|e| e.to_string())
    }

    async fn on_settled(&self, keys: &ClusterAction, result: &Result<Option<ClusterId>, String>) {
        if let Err(err) = result
            && let Ok(state) = crate::launcher::state()
        {
            state
                .services
                .events
                .notify(t("scclient.mutations.action_failed"))
                .body(err)
                .error()
                .send();
        }
        let enabled_only = match keys {
            ClusterAction::Batch(actions) => actions
                .iter()
                .all(|a| matches!(a, ClusterAction::SetArtifactEnabled { .. })),
            keys => matches!(keys, ClusterAction::SetArtifactEnabled { .. }),
        };
        if enabled_only {
            invalidate_enabled_flag_queries().await;
        } else {
            invalidate_cluster_queries().await;
        }
    }
}

async fn folder_pack_op<F, Fut>(
    state: &scclient_core::LauncherState,
    cluster_id: ClusterId,
    op: F,
) -> Result<(), scclient_content::ContentError>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = std::io::Result<()>>,
{
    let invalid = |err: &dyn std::fmt::Display| scclient_content::ContentError::InvalidData {
        reason: err.to_string(),
    };
    let cluster = state
        .clusters
        .get(cluster_id)
        .await
        .map_err(|e| invalid(&e))?;
    let dir = cluster.game_dir().map_err(|e| invalid(&e))?;
    op(dir).await.map_err(|e| invalid(&e))
}

fn notify_reset(
    services: &scclient_core::LauncherServices,
    report: &scclient_core::clusters::ResetReport,
) {
    if let Some(err) = &report.reinstall_error {
        let mut body = tf("scclient.mutations.reset.reinstall_failed", &[&err]);
        if let Some(backup) = &report.backup_dir {
            body.push(' ');
            body.push_str(&tf(
                "scclient.mutations.reset.moved_to",
                &[&backup.display()],
            ));
        }
        body.push(' ');
        body.push_str(t("scclient.mutations.reset.run_again"));
        services
            .events
            .notify(t("scclient.mutations.reset.unfinished"))
            .body(body)
            .error()
            .send();
        return;
    }

    let mut body = if report.kept_shared_configs {
        t("scclient.mutations.reset.done_kept_configs").to_string()
    } else {
        t("scclient.mutations.reset.done").to_string()
    };
    if let Some(backup) = &report.backup_dir {
        body.push(' ');
        body.push_str(&tf(
            "scclient.mutations.reset.moved_to",
            &[&backup.display()],
        ));
    }

    let notify = services.events.notify(t("scclient.mutations.reset.title"));
    if report.not_moved.is_empty() {
        notify.body(body).send();
    } else {
        body.push(' ');
        body.push_str(&tf(
            "scclient.mutations.reset.not_moved",
            &[&report.not_moved.join(", ")],
        ));
        notify.body(body).error().send();
    }
}

pub fn use_cluster_mutation() -> UseMutation<ClusterMutation> {
    use_mutation(Mutation::new(ClusterMutation))
}
