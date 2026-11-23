use std::sync::Arc;

use csclient_db::DbPool;
use tokio::sync::Mutex;

use crate::LauncherResult;
use crate::clusters::ClusterManager;
use crate::images::ImageCacheStore;
use crate::settings::{LauncherSettings, store};
use crate::versions::VersionsManager;
use csclient_auth::AuthService;
use csclient_common::paths;
use csclient_content::bundles::{BundlesManager, Prefetches};
use csclient_content::packages::provider::PackageProviderRegistry;
use csclient_discord::DiscordRpc;
use csclient_events::EventBus;
use csclient_java::JavaService;
use csclient_mc::MetadataStore;
use csclient_net::RequestClient;

#[derive(Clone)]
pub struct LauncherServices {
    pub events: EventBus,
    pub requester: RequestClient,
    pub db: DbPool,
    pub packages: PackageProviderRegistry,
    pub prefetches: Arc<Prefetches>,
}

pub struct LauncherState {
    pub services: LauncherServices,
    pub settings: parking_lot::RwLock<LauncherSettings>,
    pub auth: Arc<AuthService>,
    pub java: JavaService,
    pub clusters: ClusterManager,
    pub metadata: Mutex<MetadataStore>,
    pub bundles: Arc<BundlesManager>,
    pub versions: Arc<VersionsManager>,
    pub images: ImageCacheStore,
    pub games: crate::game::GameProcessManager,
    pub discord: DiscordRpc,
}

impl LauncherServices {
    #[must_use]
    pub fn content(&self) -> csclient_content::ContentCtx {
        csclient_content::ContentCtx::new(
            self.db.clone(),
            self.requester.clone(),
            self.events.clone(),
            self.packages.clone(),
            self.prefetches.clone(),
        )
    }

    /// Both fields are refcounted handles so building this per call is cheap
    #[must_use]
    pub fn mc(&self) -> csclient_mc::McCtx {
        csclient_mc::McCtx::new(self.requester.clone(), self.events.clone())
    }
}

impl LauncherState {
    /// No side effects beyond opening the database and reading settings the
    /// background startup work is [`run_startup_tasks`] which the caller runs
    #[tracing::instrument(skip(events))]
    pub async fn new(events: EventBus) -> LauncherResult<Arc<Self>> {
        let services = LauncherServices {
            events,
            db: csclient_db::connect(paths::database_file()?).await?,
            // Defaults because loading settings needs the event bus already inside
            // `services` Real config is pushed in below before any request
            requester: RequestClient::new(csclient_net::NetConfig::default())?,
            packages: PackageProviderRegistry::new(),
            prefetches: Arc::default(),
        };

        let settings = store::load_settings(Some(&services.events)).await;

        csclient_common::consent::init(settings.declined_tos);
        let net_config = crate::settings::net_config(&settings);
        csclient_csplus::set_base_url(&net_config.plus_api_url);
        services.requester.set_config(net_config);
        let auth =
            Arc::new(AuthService::load(services.requester.clone(), services.events.clone()).await?);
        let java = JavaService::new(
            std::sync::Arc::new(crate::java_store::SqlJavaStore::new(services.db.clone())),
            services.requester.clone(),
            services.events.clone(),
        );
        let discord = DiscordRpc::spawn(settings.discord_enabled);

        let clusters = ClusterManager::new(services.db.clone());

        let state = Arc::new(Self {
            services,
            settings: parking_lot::RwLock::new(settings),
            auth,
            java,
            clusters,
            metadata: Mutex::new(MetadataStore::new()),
            bundles: Arc::new(BundlesManager::new()),
            versions: Arc::new(VersionsManager::from_cache().await),
            images: ImageCacheStore::new(),
            games: crate::game::GameProcessManager::new(),
            discord,
        });

        Ok(state)
    }
}

/// Java archives are the only downloads big enough for a leaked scratch file to
/// matter and they land flat in the java dir so this needs no recursion
async fn sweep_java_scratch_files() {
    let dir = match paths::java_dir() {
        Ok(dir) => dir,
        Err(err) => {
            tracing::warn!("could not resolve the java dir to sweep: {err:#}");
            return;
        }
    };

    if let Err(err) = csio::sweep_temp_files(&dir).await {
        tracing::warn!("java scratch file sweep failed: {err:#}");
    }
}

pub fn run_startup_tasks(state: &Arc<LauncherState>) {
    let crash_data = Arc::clone(state);
    tokio::spawn(async move {
        crate::game::crashdata::load(&crash_data.services.requester).await;
    });

    let background = Arc::clone(state);
    tokio::spawn(async move {
        sweep_java_scratch_files().await;
        crate::migration::launchers::sweep_instance_archives().await;
        background.clusters.sweep_trash().await;
        if let Err(err) = csclient_content::packages::store::undo::recover_staged_undo_files(
            &background.services.content(),
        )
        .await
        {
            tracing::warn!("staged Undo file recovery failed: {err:#}");
        }

        let recovery = match crate::recovery::reconstruct_from_disk(&background).await {
            Ok(report) => report,
            Err(err) => {
                tracing::error!("disk recovery failed: {err:#}");
                crate::recovery::RecoveryReport::default()
            }
        };

        crate::game::recover_sessions(&background).await;
        crate::game::split_dedicated_packs(&background).await;
        crate::bisect::resume_unfinished_bisects(&background).await;

        let content = background.services.content();
        let (versions_res, bundles_res, _, _) = tokio::join!(
            background.versions.sync(&background.services),
            background.bundles.sync(&content),
            csclient_content::packages::load_bad_mods(&content),
            csclient_content::packages::load_bad_modpacks(&content),
        );
        if let Err(err) = versions_res {
            tracing::error!("versions manifest sync failed: {err:#}");
        }
        if let Err(err) = crate::clusters::record_new_versions(&background).await {
            tracing::warn!("could not record new versions for migration: {err:#}");
        }
        if let Err(err) = bundles_res {
            tracing::error!("bundle catalog sync failed: {err:#}");
        }

        let art = Arc::clone(&background);
        tokio::spawn(async move {
            crate::versions::prefetch_version_art(&art).await;
        });

        if recovery.did_recover()
            && let Err(err) = crate::recovery::restore_bundle_tracking(&background).await
        {
            tracing::warn!("bundle tracking restore failed: {err:#}");
        }

        if let Err(err) = crate::clusters::apply_remote_migrations(&background).await {
            tracing::error!("cluster migrations failed: {err:#}");
        }

        if let Err(err) = crate::clusters::ensure_from_versions(&background).await {
            tracing::error!("versions cluster provisioning failed: {err:#}");
        } else {
            background
                .services
                .events
                .signal(csclient_events::Signal::ClustersChanged);
        }

        // Must run last judging an artifact unused before recovery bundle
        // tracking and provisioning have restored their rows would evict
        // content still in use Only the row-driven half runs unattended
        if let Err(err) =
            csclient_content::packages::store::collect_unused_artifacts(&content).await
        {
            tracing::warn!("package cache cleanup failed: {err:#}");
        }

        background
            .services
            .events
            .signal(csclient_events::Signal::SyncComplete);
    });
}

pub async fn shutdown(state: &LauncherState) {
    tracing::info!("shutting the launcher down");
    state.discord.shutdown();
    state.services.db.close().await;
}
