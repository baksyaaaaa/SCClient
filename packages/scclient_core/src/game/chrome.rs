//! Chrome Client in every game this launcher starts.
//!
//! The mod bundles install Polyfrost's PolyPlus (Modrinth `Iw9mZi4a`), whose start screen has the
//! store wardrobe, the Marketplace tab, Polyfrost social and the News tab. Before each Fabric launch
//! this swaps it for Chrome Client's own copy of PolyPlus ("ChromePoly", no Marketplace or News,
//! Chrome cosmetics, social through the Chrome website) plus the Chrome Client mod that draws the
//! cosmetics. Both come from the GitHub Releases of `baksyaaaaa/ChromeMod`.
//!
//! Releases are checked once per launcher run per Minecraft version; the downloaded jars are kept,
//! so a launch while GitHub is unreachable still uses the last ones.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use scclient_common::domain::{ContentType, GameLoader, ProviderId};
use scclient_common::paths;
use scclient_content::packages::PackageStore;

use crate::state::LauncherState;

const REPO: &str = "baksyaaaaa/ChromeMod";
/// Polyfrost's PolyPlus on Modrinth: it clashes with ChromePoly (same mod id), so it is switched off
const POLYPLUS_PROJECT: &str = "Iw9mZi4a";
const POLY_TAG: &str = "polyplus-latest";
/// Minecraft versions ChromePoly is built for
const POLY_VERSIONS: &[&str] = &["1.21.11", "26.1", "26.2", "26.3"];

const POLY_PREFIX: &str = "ChromePoly-";
const MOD_PREFIX: &str = "chrome-client-";

/// mc version -> jars found this run (so GitHub is asked once per run)
static CHECKED: parking_lot::Mutex<Option<HashMap<String, Vec<PathBuf>>>> =
    parking_lot::Mutex::new(None);

fn cache_dir() -> Option<PathBuf> {
    paths::data_dir().ok().map(|d| d.join("chrome"))
}

/// Which build of the Chrome Client mod fits: 1.21.11 has its own, 26.x share the 26.3 build
fn mod_build(mc: &str) -> Option<&'static str> {
    if mc == "1.21.11" {
        Some("1.21.11")
    } else if mc.starts_with("26.") {
        Some("26.3")
    } else {
        None
    }
}

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    updated_at: String,
}

/// Makes sure `cluster_id` runs ChromePoly + Chrome Client instead of Polyfrost's PolyPlus.
/// Never fails a launch: anything that goes wrong is logged and the game starts as it was.
pub(crate) async fn apply(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    loader: GameLoader,
    mc: &str,
    game_dir: &std::path::Path,
) {
    if !matches!(loader, GameLoader::Fabric) {
        return;
    }
    // the wardrobe picks go to the game (or the game's newer picks come back)
    crate::chrome_service::sync_to_game(game_dir).await;
    let poly = POLY_VERSIONS.contains(&mc);
    let build = mod_build(mc);
    if !poly && build.is_none() {
        return;
    }

    let jars = match jars_for(state, mc, poly, build).await {
        Some(jars) if !jars.is_empty() => jars,
        _ => {
            tracing::warn!(cluster_id, mc, "no Chrome Client jars available; launching as is");
            return;
        }
    };
    // SC logo and name in the menu mod (done once per downloaded jar; a branded jar is skipped)
    for jar in jars.iter().filter(|j| name_of(j).starts_with(POLY_PREFIX)) {
        match crate::game::scbrand::brand_jar(jar).await {
            Ok(true) => tracing::info!(jar = %jar.display(), "put the SC branding in the menu mod"),
            Ok(false) => {}
            Err(err) => tracing::warn!(jar = %jar.display(), error = %err, "could not brand the menu mod"),
        }
    }
    if let Err(err) = link(state, cluster_id, &jars).await {
        tracing::warn!(cluster_id, error = %err, "could not set up Chrome Client");
    }
}

async fn jars_for(
    state: &Arc<LauncherState>,
    mc: &str,
    poly: bool,
    build: Option<&str>,
) -> Option<Vec<PathBuf>> {
    if let Some(found) = CHECKED.lock().as_ref().and_then(|m| m.get(mc).cloned()) {
        return Some(found);
    }
    let dir = cache_dir()?;
    tokio::fs::create_dir_all(&dir).await.ok()?;

    let releases = fetch_releases(state).await;
    let mut found = Vec::new();

    if let Some(releases) = &releases {
        // ChromePoly: fixed release, fixed file name per version
        if poly && let Some(asset) = releases
            .iter()
            .filter(|r| r.tag_name == POLY_TAG)
            .flat_map(|r| &r.assets)
            .find(|a| a.name == format!("{POLY_PREFIX}{mc}.jar"))
            && let Some(path) = download(state, &dir, asset).await
        {
            found.push(path);
        }
        // the mod: newest "build-N" release that has a jar for this version
        if let Some(build) = build
            && let Some(asset) = releases
                .iter()
                .filter(|r| !r.draft && r.tag_name != POLY_TAG)
                .flat_map(|r| &r.assets)
                .find(|a| {
                    a.name.starts_with(&format!("{MOD_PREFIX}{build}"))
                        && a.name.ends_with(".jar")
                        && !a.name.contains("-sources")
                })
            && let Some(path) = download(state, &dir, asset).await
        {
            found.push(path);
        }
    }

    // offline (or GitHub said no): use what was downloaded before
    if found.is_empty() {
        let mut cached = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_poly = poly && name == format!("{POLY_PREFIX}{mc}.jar");
                let is_mod = build.is_some_and(|b| name.starts_with(&format!("{MOD_PREFIX}{b}")) && name.ends_with(".jar"));
                if is_poly || is_mod {
                    cached.push(entry.path());
                }
            }
        }
        // only the newest mod jar
        cached.sort();
        let mut keep: Vec<PathBuf> = cached.iter().filter(|p| name_of(p).starts_with(POLY_PREFIX)).cloned().collect();
        if let Some(last) = cached.iter().filter(|p| name_of(p).starts_with(MOD_PREFIX)).last() {
            keep.push(last.clone());
        }
        found = keep;
    } else if releases.is_some() {
        CHECKED.lock().get_or_insert_with(HashMap::new).insert(mc.to_string(), found.clone());
    }
    Some(found)
}

fn name_of(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

async fn fetch_releases(state: &Arc<LauncherState>) -> Option<Vec<Release>> {
    let res = state
        .services
        .requester
        .http()
        .get(format!("https://api.github.com/repos/{REPO}/releases?per_page=30"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "SCClient")
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        tracing::warn!(status = %res.status(), "GitHub did not list the Chrome Client releases");
        return None;
    }
    res.json::<Vec<Release>>().await.ok()
}

/// Downloads an asset unless the same upload is already cached (a `.stamp` file holds its upload time)
async fn download(state: &Arc<LauncherState>, dir: &std::path::Path, asset: &Asset) -> Option<PathBuf> {
    let path = dir.join(&asset.name);
    let stamp = dir.join(format!("{}.stamp", asset.name));
    if path.is_file()
        && tokio::fs::read_to_string(&stamp).await.ok().as_deref() == Some(asset.updated_at.as_str())
    {
        return Some(path);
    }
    let res = state
        .services
        .requester
        .http()
        .get(&asset.browser_download_url)
        .header("User-Agent", "SCClient")
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        tracing::warn!(name = %asset.name, status = %res.status(), "could not download");
        return path.is_file().then_some(path);
    }
    let bytes = res.bytes().await.ok()?;
    let tmp = dir.join(format!("{}.part", asset.name));
    tokio::fs::write(&tmp, &bytes).await.ok()?;
    tokio::fs::rename(&tmp, &path).await.ok()?;
    let _ = tokio::fs::write(&stamp, &asset.updated_at).await;
    // older mod builds of the same version are no longer needed
    if asset.name.starts_with(MOD_PREFIX)
        && let Some(build) = asset.name.strip_prefix(MOD_PREFIX).and_then(|r| r.split('-').next())
        && let Ok(mut rd) = tokio::fs::read_dir(dir).await
    {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let n = entry.file_name().to_string_lossy().to_string();
            if n != asset.name && n.starts_with(&format!("{MOD_PREFIX}{build}")) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    Some(path)
}

/// Links the jars into the cluster, switches off Polyfrost's PolyPlus and any older Chrome jars
async fn link(state: &Arc<LauncherState>, cluster_id: i64, jars: &[PathBuf]) -> crate::LauncherResult<()> {
    let ctx = state.services.content();
    let mut keep = Vec::new();
    for jar in jars {
        let hash = scio::normalize_hash(&scio::sha1_file(jar).await?);
        let linked = PackageStore::list_linked_artifacts(cluster_id, &ctx).await?;
        match linked.iter().find(|l| l.hash == hash) {
            Some(l) if l.enabled => {}
            Some(_) => {
                PackageStore::set_artifact_enabled_to(cluster_id, &hash, true, &ctx).await?;
            }
            None => {
                PackageStore::import_local_file(jar, ContentType::Mod, cluster_id, &ctx).await?;
                // an import may land switched off if the cluster had it disabled before
                let _ = PackageStore::set_artifact_enabled_to(cluster_id, &hash, true, &ctx).await;
            }
        }
        keep.push(hash);
    }

    let has_poly = jars.iter().any(|j| name_of(j).starts_with(POLY_PREFIX));
    for l in PackageStore::list_linked_artifacts(cluster_id, &ctx).await? {
        if !l.enabled || l.content_type != ContentType::Mod || keep.contains(&l.hash) {
            continue;
        }
        let polyfrost_plus = l.provider == Some(ProviderId::Modrinth)
            && l.project_id.as_deref() == Some(POLYPLUS_PROJECT);
        let old_chrome = l.file_name.starts_with(POLY_PREFIX) || l.file_name.starts_with(MOD_PREFIX);
        if (polyfrost_plus && has_poly) || old_chrome {
            PackageStore::set_artifact_enabled_to(cluster_id, &l.hash, false, &ctx).await?;
            tracing::info!(cluster_id, file = %l.file_name, "switched off for Chrome Client");
        }
    }
    Ok(())
}
