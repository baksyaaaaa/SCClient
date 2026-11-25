//! The wardrobe: SC Client's hats, capes and wings (the same list as the in-game mod).
//! Picks are saved by the launcher, sent to the SC Client service, and put into each game before it starts.

use bytes::Bytes;
use freya::prelude::*;
use scclient_core::chrome_service::{self, CATEGORIES, Picks};
use scclient_scplus::api;

use crate::assets::AppAssets;
use crate::components::{Button, PlayerModel, ScrollArea, use_my_uuid};
use crate::hooks::wardrobe::CosmeticBundle;
use crate::launcher;
use crate::theme::{colors, rounded, text_size};

const PER_ROW: usize = 4;

#[derive(PartialEq)]
pub struct ChromeWardrobe;

impl Component for ChromeWardrobe {
    fn render(&self) -> impl IntoElement {
        let mut picks = use_state(|| None::<Picks>);
        let category = use_state(|| 0usize);
        let status = use_state(String::new);

        use_hook(move || {
            spawn(async move {
                let loaded = launcher::off_ui(chrome_service::load_picks()).await;
                picks.set(Some(loaded));
            });
        });

        let current = *picks.read();
        let cat = *category.read();
        let names = chrome_service::items(cat);
        let equipped = current.map(|p| p.get(cat)).unwrap_or(0);

        // shows the pick at once; saving and the SC Client service happen in the background
        let wear = move |p: Picks, message: &str| {
            let (mut picks, mut status) = (picks, status);
            picks.set(Some(p));
            status.set(message.to_string());
            if let Ok(state) = launcher::state() {
                chrome_service::apply_picks(state, p);
            }
        };
        let equip = move |index: usize| {
            let base = picks.peek().unwrap_or_default();
            wear(base.with(cat, index), "");
        };
        let clear = move |_| wear(Picks::default(), "Took everything off.");

        let mut tabs = rect().horizontal().spacing(8.).cross_align(Alignment::Center);
        for (i, name) in CATEGORIES.iter().enumerate() {
            let mut category = category;
            let button = Button::new().text(*name).on_press(move |_| category.set(i));
            tabs = tabs.child(if i == cat { button.primary() } else { button.secondary() });
        }
        tabs = tabs.child(Button::new().ghost().text("Clear all").on_press(clear));

        let mut grid = rect().vertical().spacing(8.).width(Size::fill());
        for row in (0..names.len()).collect::<Vec<_>>().chunks(PER_ROW) {
            let mut line = rect().horizontal().spacing(8.).width(Size::fill());
            for &index in row {
                let button = Button::new()
                    .width(Size::px(190.))
                    .text(names[index])
                    .on_press(move |_| equip(index));
                line = line.child(if index == equipped { button.primary() } else { button.secondary() });
            }
            grid = grid.child(line);
        }

        let me = use_my_uuid().map(|id| id.to_string()).unwrap_or_default();
        let (cape, cosmetics) = preview_parts(current.unwrap_or_default());
        let preview = rect()
            .width(Size::fill())
            .height(Size::px(460.))
            .corner_radius(rounded(12.))
            .background(colors::card())
            .overflow(Overflow::Clip)
            .child(PlayerModel::new(me).yaw(2.6).zoom(0.72).focus_y(4.).cape(cape).cosmetics(cosmetics).width(Size::fill()).height(Size::fill()));

        let equipped_text = match current {
            None => "Loading...".to_string(),
            Some(_) if equipped == 0 => "Nothing equipped".to_string(),
            Some(_) => format!("Equipped: {}", names[equipped]),
        };

        rect().vertical().width(Size::fill()).height(Size::fill()).overflow(Overflow::Clip).child(
            ScrollArea::new().width(Size::fill()).height(Size::fill()).child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 40., 40., 40.))
                    .spacing(18.)
                    .child(header())
                    .child(tabs)
                    .child(
                        label()
                            .text(equipped_text)
                            .font_size(text_size(14.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(grid)
                    .child(preview)
                    .maybe_child((!status.read().is_empty()).then(|| {
                        label()
                            .text(status.read().clone())
                            .font_size(text_size(13.))
                            .color(colors::fg_secondary())
                            .into_element()
                    }))
                    .child(note()),
            ),
        )
    }
}

/// The preview's cape texture and hat / wing models for these picks (built from the SC mod's own cosmetics, assets/sc_cosmetics)
fn preview_parts(picks: Picks) -> (Option<Bytes>, Vec<(api::BodySlot, CosmeticBundle)>) {
    let cape = (picks.capes > 0).then(|| AppAssets::get_bytes(&format!("sc_cosmetics/cape_{}.png", picks.capes))).flatten();
    let model = |name: String| CosmeticBundle {
        geometry: AppAssets::get_bytes(&format!("sc_cosmetics/{name}.geo.json")),
        texture: AppAssets::get_bytes(&format!("sc_cosmetics/{name}.png")),
        ..Default::default()
    };
    let mut cosmetics = Vec::new();
    if picks.hats > 0 {
        cosmetics.push((api::BodySlot::Hat, model(format!("hat_{}", picks.hats))));
    }
    if picks.wings > 0 {
        cosmetics.push((api::BodySlot::Wings, model(format!("wings_{}", picks.wings))));
    }
    (cape, cosmetics)
}

fn header() -> Element {
    rect()
        .vertical()
        .spacing(2.)
        .child(
            label()
                .text("Wardrobe")
                .font_size(text_size(32.))
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text("SC Client cosmetics. Everyone who plays with SC Client sees them on you.")
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn note() -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(14.))
        .corner_radius(rounded(12.))
        .background(colors::card())
        .child(
            label()
                .text(
                    "Your picks are put into the game when you press Play (Fabric versions). \
                     If you change them in game, the launcher picks that up the next time you play.",
                )
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}
