pub const DUMMY_REPLACE_NEWLINE: &str = "\n";

pub const MICROSOFT_CLIENT_ID: &str = "5e0662c2-5c39-4893-9e50-008a14d4f4e7";
// Same scopes as the Chrome Client launcher (minecraft_launcher_lib), which signs in fine with this Azure app
pub const MINECRAFT_SCOPES: &str = "XboxLive.signin offline_access";
pub const CURSEFORGE_API_KEY: &str = "$2a$10$6utA1UNSmFPrE/Lh7b7ndeeGmiOkjKNY8kpFB0fsmE/d42ZAfFgCe";
pub const DISCORD_CLIENT_ID: &str = "1426999264633946334";

pub const MODRINTH_API_URL: &str = "https://api.modrinth.com";
pub const MODRINTH_CDN_PREFIX: &str = "https://cdn.modrinth.com/data/";
pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com/v1";
pub const CURSEFORGE_GAME_ID: u32 = 432;
pub const METADATA_API_URL: &str = "https://meta.polyfrost.org";
pub const MCLOGS_API_URL: &str = "https://api.mclo.gs/1";
pub const SKYCLIENT_BASE_URL: &str =
    "https://raw.githubusercontent.com/SkyblockClient/SkyblockClient-REPO/refs/heads/main/v1";
pub const META_URL_BASE: &str = "https://data-v2.polyfrost.org";
pub const TOS_URL: &str = "https://polyfrost.org/legal/terms";
pub const PRIVACY_URL: &str = "https://polyfrost.org/legal/privacy";
pub const PLUS_BACKEND_URL: &str = "https://plus.polyfrost.org";
pub const CRASH_DATA_URL: &str =
    "https://raw.githubusercontent.com/Polyfrost/CrashData/refs/heads/main/crashes.json";

pub const DISCORD_URL: &str = "https://discord.com/invite/polyfrost";

/// Crash reports are off unless a Sentry DSN of your own is set at build time (SCCLIENT_SENTRY_DSN);
/// the built-in one belonged to Polyfrost.
pub const SENTRY_DSN: &str = match option_env!("SCCLIENT_SENTRY_DSN") {
    Some(dsn) => dsn,
    None => "",
};

pub const LAUNCHER_LABEL: &str = concat!(
    "SCClient v",
    env!("CARGO_PKG_VERSION"),
    cfg_select! {
        debug_assertions => " (debug)",
        _ => " (release)"
    }
);

pub const TARGET_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "osx",
    target_os = "linux" => "linux"
};

pub const NATIVE_ARCH: &str = cfg_select! {
    target_arch = "x86" => "32",
    target_arch = "x86_64" => "64",
    _ => "64"
};

pub const JAVA_BIN: &str = if cfg!(windows) { "javaw.exe" } else { "java" };

pub const ARCH_WIDTH: &str = if cfg!(target_pointer_width = "64") {
    "64"
} else {
    "32"
};

pub const LINE_ENDING: &str = if cfg!(windows) { "\r\n" } else { "\n" };
pub const CLASSPATH_SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };
