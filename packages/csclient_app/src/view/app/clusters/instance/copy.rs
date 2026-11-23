use csclient_cluster::naming::validate_instance_name;

use super::data::Picks;
use super::model::{
    FolderChoice, ImportMode, ImportSource, ModpackOrigin, Step, TypeChoice, Wizard,
};
use crate::i18n::{t, tf};
use crate::utils::count_label;

pub fn heading(picks: &Picks) -> (&'static str, String) {
    match picks.step {
        Step::Type => (
            t("csclient.clusters.instance.copy.type.title"),
            t("csclient.clusters.instance.copy.type.subtitle").to_string(),
        ),
        Step::Folder => (
            t("csclient.clusters.instance.copy.folder.title"),
            t("csclient.clusters.instance.copy.folder.subtitle").to_string(),
        ),
        Step::Loader => (
            t("csclient.clusters.instance.copy.loader.title"),
            match &picks.versions.chosen {
                Some(version) => tf(
                    "csclient.clusters.instance.copy.loader.subtitle_with_version",
                    &[&version],
                ),
                None => t("csclient.clusters.instance.copy.loader.subtitle").to_string(),
            },
        ),
        Step::Version => (
            t("csclient.clusters.instance.copy.version.title"),
            match picks.choice {
                TypeChoice::CSClient => {
                    t("csclient.clusters.instance.copy.version.subtitle.csclient").to_string()
                }
                TypeChoice::Scratch | TypeChoice::Modpack | TypeChoice::Import => {
                    t("csclient.clusters.instance.copy.version.subtitle.any").to_string()
                }
            },
        ),
        Step::Bundles => match (picks.bundle_target(), &picks.improve_targets) {
            (Some(target), Some(targets)) => (
                t("csclient.clusters.instance.copy.bundles.title"),
                tf(
                    "csclient.clusters.instance.copy.bundles.subtitle_improve",
                    &[&target.name, &(picks.bundle_page + 1), &targets.len()],
                ),
            ),
            (None, Some(_)) => (
                t("csclient.clusters.instance.copy.bundles.title"),
                t("csclient.clusters.instance.copy.note.import_mode.improve").to_string(),
            ),
            _ => (
                t("csclient.clusters.instance.copy.bundles.title"),
                t("csclient.clusters.instance.copy.bundles.subtitle").to_string(),
            ),
        },
        Step::Customize => (
            t("csclient.clusters.instance.copy.customize.title"),
            t("csclient.clusters.instance.copy.customize.subtitle").to_string(),
        ),
        Step::Modpack => (
            t("csclient.clusters.instance.copy.modpack.title"),
            t("csclient.clusters.instance.copy.modpack.subtitle").to_string(),
        ),
        Step::ImportSource => (
            t("csclient.clusters.instance.copy.import_source.title"),
            t("csclient.clusters.instance.copy.import_source.subtitle").to_string(),
        ),
        Step::ImportMode if picks.choice == TypeChoice::Modpack => (
            t("csclient.clusters.instance.copy.import_mode.title_modpack"),
            t("csclient.clusters.instance.copy.import_mode.subtitle_modpack").to_string(),
        ),
        Step::ImportMode => (
            t("csclient.clusters.instance.copy.import_mode.title"),
            t("csclient.clusters.instance.copy.import_mode.subtitle").to_string(),
        ),
        Step::Import => (
            t("csclient.clusters.instance.copy.import.title"),
            t("csclient.clusters.instance.copy.import.subtitle").to_string(),
        ),
    }
}

pub fn footer_note(picks: &Picks) -> String {
    match picks.step {
        Step::Type => match picks.choice {
            TypeChoice::CSClient => {
                t("csclient.clusters.instance.copy.note.type.csclient").to_string()
            }
            TypeChoice::Scratch => {
                t("csclient.clusters.instance.copy.note.type.scratch").to_string()
            }
            TypeChoice::Modpack => {
                t("csclient.clusters.instance.copy.note.type.modpack").to_string()
            }
            TypeChoice::Import => {
                t("csclient.clusters.instance.copy.note.type.import").to_string()
            }
        },
        Step::Folder => match &picks.folder {
            FolderChoice::Shared => {
                t("csclient.clusters.instance.copy.note.folder.shared").to_string()
            }
            FolderChoice::Dedicated => {
                t("csclient.clusters.instance.copy.note.folder.dedicated").to_string()
            }
            FolderChoice::Custom(group) => tf(
                "csclient.clusters.instance.copy.note.folder.custom",
                &[group],
            ),
        },
        Step::Loader => match (&picks.versions.chosen, picks.loader.chosen) {
            (Some(version), None) => {
                tf("csclient.clusters.instance.copy.note.loader", &[&version])
            }
            _ => picks.loader_label(),
        },
        Step::Version => match &picks.versions.chosen {
            Some(version) => tf(
                "csclient.clusters.instance.copy.note.version.chosen",
                &[&version],
            ),
            None => t("csclient.clusters.instance.copy.note.version.none").to_string(),
        },
        Step::Bundles if picks.improving => screening_note(picks),
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            let total = picks.bundles.archives.len();
            if taken == 0 {
                t("csclient.clusters.instance.copy.note.bundles.none").to_string()
            } else {
                tf(
                    "csclient.clusters.instance.copy.note.bundles.some",
                    &[&taken, &total],
                )
            }
        }
        Step::Customize => t("csclient.clusters.instance.copy.note.customize").to_string(),
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => {
                t("csclient.clusters.instance.copy.note.modpack.browse").to_string()
            }
            ModpackOrigin::File => {
                t("csclient.clusters.instance.copy.note.modpack.file").to_string()
            }
        },
        Step::ImportSource => match picks.import_source {
            ImportSource::File => {
                t("csclient.clusters.instance.copy.note.import_source.file").to_string()
            }
            ImportSource::Code => {
                t("csclient.clusters.instance.copy.note.import_source.code").to_string()
            }
            ImportSource::Launcher => {
                t("csclient.clusters.instance.copy.note.import_source.launcher").to_string()
            }
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs if picks.choice == TypeChoice::Modpack => {
                t("csclient.clusters.instance.copy.note.import_mode.as_is_modpack").to_string()
            }
            ImportMode::AsIs => {
                t("csclient.clusters.instance.copy.note.import_mode.as_is").to_string()
            }
            ImportMode::Improve => {
                t("csclient.clusters.instance.copy.note.import_mode.improve").to_string()
            }
        },
        Step::Import if picks.finishing => screening_note(picks),
        Step::Import => match picks.import_count {
            0 => t("csclient.clusters.instance.copy.note.import.none").to_string(),
            1 => t("csclient.clusters.instance.copy.note.import.one").to_string(),
            n => tf("csclient.clusters.instance.copy.note.import.other", &[&n]),
        },
    }
}

fn screening_note(picks: &Picks) -> String {
    let pack = picks.choice == TypeChoice::Modpack;
    match &picks.import_screening {
        None if picks.import_screening_error.is_some() => if pack {
            t("csclient.clusters.instance.copy.note.import_review.unchecked_modpack")
        } else {
            t("csclient.clusters.instance.copy.note.import_review.unchecked")
        }
        .to_string(),
        None => t("csclient.clusters.instance.copy.note.import_review.checking").to_string(),
        Some(_) => match (review_parts(picks).as_slice(), pack) {
            ([], true) => {
                t("csclient.clusters.instance.copy.note.import_review.none_modpack").to_string()
            }
            ([], false) => t("csclient.clusters.instance.copy.note.import_review.none").to_string(),
            (parts, true) => tf(
                "csclient.clusters.instance.copy.note.import_review.parts_modpack",
                &[&parts.join(", ")],
            ),
            (parts, false) => tf(
                "csclient.clusters.instance.copy.note.import_review.parts",
                &[&parts.join(", ")],
            ),
        },
    }
}

pub fn step_value(wizard: Wizard, picks: &Picks, step: Step) -> String {
    match step {
        Step::Type => match picks.choice {
            TypeChoice::CSClient => "CSClient".to_string(),
            TypeChoice::Scratch => t("csclient.clusters.instance.copy.value.scratch").to_string(),
            TypeChoice::Modpack => t("csclient.clusters.instance.copy.value.modpack").to_string(),
            TypeChoice::Import => t("csclient.clusters.instance.copy.value.import").to_string(),
        },
        Step::Folder => match &picks.folder {
            FolderChoice::Shared => t("csclient.clusters.instance.copy.value.shared").to_string(),
            FolderChoice::Dedicated => {
                t("csclient.clusters.instance.copy.value.dedicated").to_string()
            }
            FolderChoice::Custom(group) => group.clone(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => {
            picks.versions.chosen.clone().unwrap_or_else(|| {
                t("csclient.clusters.instance.copy.value.not_chosen").to_string()
            })
        }
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            if taken == 0 {
                t("csclient.clusters.instance.copy.value.none").to_string()
            } else {
                tf("csclient.clusters.instance.copy.value.selected", &[&taken])
            }
        }
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => t("csclient.clusters.instance.copy.value.browse").to_string(),
            ModpackOrigin::File => t("csclient.clusters.instance.copy.value.file").to_string(),
        },
        Step::ImportSource => match picks.import_source {
            ImportSource::File => t("csclient.clusters.instance.copy.value.file").to_string(),
            ImportSource::Code => t("csclient.clusters.instance.copy.value.code").to_string(),
            ImportSource::Launcher => {
                t("csclient.clusters.instance.copy.value.launcher").to_string()
            }
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs => t("csclient.clusters.instance.copy.value.as_is").to_string(),
            ImportMode::Improve => t("csclient.clusters.instance.copy.value.improved").to_string(),
        },
        Step::Import => match picks.import_count {
            0 => t("csclient.clusters.instance.copy.value.none_selected").to_string(),
            n => tf("csclient.clusters.instance.copy.value.selected", &[&n]),
        },
        Step::Customize => match wizard.details.tags.read().len() {
            0 => t("csclient.clusters.instance.copy.value.optional").to_string(),
            1 => t("csclient.clusters.instance.copy.value.tags.one").to_string(),
            many => tf(
                "csclient.clusters.instance.copy.value.tags.other",
                &[&many],
            ),
        },
    }
}

fn review_parts(picks: &Picks) -> Vec<String> {
    [
        (
            picks.flagged_count(),
            t("csclient.clusters.instance.copy.value.flagged.one"),
            t("csclient.clusters.instance.copy.value.flagged.other"),
        ),
        (
            picks.unpublished_count(),
            t("csclient.clusters.instance.copy.value.unpublished.one"),
            t("csclient.clusters.instance.copy.value.unpublished.other"),
        ),
        (
            picks.unconfirmed_count(),
            t("csclient.clusters.instance.copy.value.unconfirmed.one"),
            t("csclient.clusters.instance.copy.value.unconfirmed.other"),
        ),
    ]
    .into_iter()
    .filter(|(count, _, _)| *count > 0)
    .map(|(count, one, many)| count_label(count, one, many))
    .collect()
}

pub fn is_ready(picks: &Picks) -> bool {
    match picks.step {
        Step::Bundles if picks.improving => {
            picks
                .improve_targets
                .as_ref()
                .is_some_and(|targets| targets.is_empty() || picks.bundles.loaded)
        }
        Step::Type | Step::Folder | Step::Bundles | Step::Modpack => true,
        Step::Loader => picks.loader.chosen.is_some_and(|loader| picks.fits(loader)),
        Step::Version => {
            picks.versions.chosen.is_some()
                && (picks.choice == TypeChoice::Scratch
                    || picks.loader.chosen.is_some_and(|loader| picks.fits(loader)))
        }
        Step::Customize => {
            validate_instance_name(&picks.name).is_ok()
                && picks.loader.chosen.is_none_or(|loader| picks.fits(loader))
        }
        Step::ImportSource => {
            picks.import_source != ImportSource::Code
                || csclient_core::sharing::ShareCode::parse(&picks.share_code).is_some()
        }
        Step::ImportMode => true,
        Step::Import => {
            picks.import_count > 0 && (!picks.improving || picks.improve_targets.is_some())
        }
    }
}
