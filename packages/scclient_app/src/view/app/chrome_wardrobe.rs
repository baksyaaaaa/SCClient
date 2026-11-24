//! The wardrobe: Chrome Client's own hats, capes and wings (the same list as the Chrome Client mod).
//! Picks are saved by the launcher, sent to the Chrome Client service, and put into each game before it starts.

use freya::prelude::*;
use scclient_core::chrome_service::{self, CATEGORIES, Picks};

use crate::components::{Button, ScrollArea};
use crate::launcher;
use crate::theme::{colors, rounded, text_size};

const PER_ROW: usize = 4;

#[derive(PartialEq)]
pub struct ChromeWardrobe;

impl Component for ChromeWardrobe {
    fn render(&self) -> impl IntoElement {
        let mut picks = use_state(|| None::<Picks>);
        let category = use_state(|| 0usize);
        let mut status = use_state(String::new);

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

        let equip = move |index: usize| {
            spawn(async move {
                let Ok(state) = launcher::state() else { return };
                match launcher::off_ui(async move { chrome_service::choose(&state, cat, index).await }).await {
                    Ok(p) => {
                        picks.set(Some(p));
                        status.set(String::new());
                    }
                    Err(err) => status.set(err),
                }
            });
        };

        let clear = move |_| {
            spawn(async move {
                let Ok(state) = launcher::state() else { return };
                match launcher::off_ui(async move { chrome_service::clear_all(&state).await }).await {
                    Ok(p) => {
                        picks.set(Some(p));
                        status.set("Took everything off.".into());
                    }
                    Err(err) => status.set(err),
                }
            });
        };

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
                .text("Chrome Client cosmetics. Everyone who plays with the Chrome Client mod sees them on you.")
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
                    "Your picks are put into the game when you press Play (Fabric versions with the Chrome Client mod). \
                     If you change them in game, the launcher picks that up the next time you play.",
                )
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}
