//! Friends and direct messages through the SC Client service - the same friends you have in game. Sign-in is Mojang's own check, using the launcher's Microsoft account.

use std::time::Duration;

use freya::prelude::*;
use scclient_core::chrome_service::{self, Message, SocialState};

use crate::components::{Avatar, Button, ScrollArea, TextInput};
use crate::launcher;
use crate::theme::{colors, rounded, text_size};

const KEEP: usize = 200;

#[derive(Clone, PartialEq)]
struct Open {
    uuid: String,
    name: String,
    faces: Vec<String>,
    team: bool,
}

#[derive(PartialEq)]
pub struct Friends;

impl Component for Friends {
    fn render(&self) -> impl IntoElement {
        let social = use_state(|| None::<SocialState>);
        let mut status = use_state(|| "Connecting to SC Client friends...".to_string());
        let mut open = use_state(|| None::<Open>);
        let mut lines = use_state(Vec::<Message>::new);
        let mut last_id = use_state(|| 0u64);
        let add_name = use_state(String::new);
        let draft = use_state(String::new);

        // keeps everything fresh while the page is open: the friend list every few seconds, the open chat more often
        use_hook(move || {
            spawn(async move {
                let mut round = 0u32;
                loop {
                    if round % 3 == 0 {
                        refresh(social, status).await;
                    }
                    let who = open.peek().clone();
                    if let Some(who) = who {
                        let after = *last_id.peek();
                        let uuid = who.uuid.clone();
                        let got = launcher::off_ui(async move {
                            let state = launcher::state().map_err(|e| e.to_string())?;
                            chrome_service::messages(&state, &uuid, after).await
                        })
                        .await;
                        // only if the same chat is still open
                        if open.peek().as_ref() == Some(&who)
                            && let Ok(new) = got
                            && !new.is_empty()
                        {
                            let top = new.iter().map(|m| m.id).max().unwrap_or(after);
                            let mut all = lines.peek().clone();
                            all.extend(new.into_iter().filter(|m| m.id > after));
                            let cut = all.len().saturating_sub(KEEP);
                            all.drain(..cut);
                            lines.set(all);
                            last_id.set(top.max(after));
                        }
                    }
                    round = round.wrapping_add(1);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        });

        let add = move |_| {
            let mut add_name = add_name;
            let name = add_name.peek().trim().to_string();
            if name.is_empty() {
                return;
            }
            add_name.set(String::new());
            act(social, status, move |state| async move { chrome_service::add_friend(&state, &name).await }, Some(
                "Friend request sent.",
            ));
        };

        let send = move |_| {
            let mut draft = draft;
            let text = draft.peek().trim().to_string();
            let Some(who) = open.peek().clone() else { return };
            if text.is_empty() {
                return;
            }
            draft.set(String::new());
            spawn(async move {
                let uuid = who.uuid.clone();
                let sent = launcher::off_ui(async move {
                    let state = launcher::state().map_err(|e| e.to_string())?;
                    chrome_service::send_message(&state, &uuid, &text).await
                })
                .await;
                if let Err(err) = sent {
                    status.set(err);
                }
            });
        };

        let snapshot = social.read().clone();
        let status_text = if status.read().is_empty() {
            "The same friends and messages as in game.".to_string()
        } else {
            status.read().clone()
        };
        let current = open.read().clone();

        // ----- left column: add, requests, friends
        let mut left = rect()
            .vertical()
            .width(Size::px(320.))
            .height(Size::fill())
            .spacing(14.)
            .child(
                rect()
                    .horizontal()
                    .spacing(8.)
                    .cross_align(Alignment::Center)
                    .child(TextInput::new(add_name).placeholder("Add a friend by username").width(Size::flex(1.0)).on_submit(
                        move |_| add(()),
                    ))
                    .child(Button::new().primary().text("Add").on_press(move |_| add(()))),
            );

        if let Some(s) = &snapshot {
            if !s.incoming.is_empty() {
                left = left.child(section("Friend requests"));
                for p in &s.incoming {
                    let (a, d) = (p.uuid.clone(), p.uuid.clone());
                    left = left.child(
                        person_row(&p.name, colors::fg_primary())
                            .child(Button::new().primary().small().text("Accept").on_press(move |_| {
                                let u = a.clone();
                                act(social, status, move |st| async move { chrome_service::friend_action(&st, "accept", &u).await }, None)
                            }))
                            .child(Button::new().ghost().small().text("Decline").on_press(move |_| {
                                let u = d.clone();
                                act(social, status, move |st| async move { chrome_service::friend_action(&st, "decline", &u).await }, None)
                            })),
                    );
                }
            }
            if !s.outgoing.is_empty() {
                left = left.child(section("Sent requests"));
                for p in &s.outgoing {
                    let c = p.uuid.clone();
                    left = left.child(
                        person_row(&p.name, colors::fg_secondary()).child(Button::new().ghost().small().text("Cancel").on_press(
                            move |_| {
                                let u = c.clone();
                                act(social, status, move |st| async move { chrome_service::friend_action(&st, "cancel", &u).await }, None)
                            },
                        )),
                    );
                }
            }
            left = left.child(section(&format!("Friends ({})", s.friends.len())));
            if s.friends.is_empty() {
                left = left.child(muted("No friends yet. Add someone who uses SC Client."));
            }
            let mut list = rect().vertical().spacing(4.).width(Size::fill());
            for f in &s.friends {
                let face_ids = if f.faces.is_empty() { vec![f.uuid.clone()] } else { f.faces.clone() };
                let pick = Open { uuid: f.uuid.clone(), name: f.name.clone(), faces: face_ids.clone(), team: f.team };
                let selected = current.as_ref().is_some_and(|o| o.uuid == f.uuid);
                let text = if f.unread > 0 { format!("{}  ({} new)", f.name, f.unread) } else { f.name.clone() };
                let button = Button::new()
                    .width(Size::fill())
                    .child(
                        rect()
                            .horizontal()
                            .spacing(10.)
                            .cross_align(Alignment::Center)
                            .child(faces_row(&face_ids, 24.))
                            .maybe_child((!f.team).then(|| dot(f.online)))
                            .child(label().text(text).font_size(text_size(14.)).color(colors::fg_primary())),
                    )
                    .on_press(move |_| {
                        open.set(Some(pick.clone()));
                        lines.set(Vec::new());
                        last_id.set(0);
                    });
                list = list.child(if selected { button.secondary() } else { button.ghost() });
            }
            left = left.child(ScrollArea::new().width(Size::fill()).height(Size::flex(1.0)).child(list));
        }

        // ----- right column: the open conversation
        let right: Element = match &current {
            None => rect()
                .width(Size::flex(1.0))
                .height(Size::fill())
                .center()
                .child(muted("Pick a friend to see your messages."))
                .into_element(),
            Some(who) => {
                let remove_uuid = who.uuid.clone();
                let mut msgs = rect().vertical().spacing(6.).width(Size::fill()).padding(Gaps::new_all(4.));
                for m in lines.read().iter() {
                    msgs = msgs.child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .child(
                                label()
                                    .text(m.name.clone())
                                    .font_size(text_size(12.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(if m.mine { colors::brand() } else { colors::fg_secondary() }),
                            )
                            .child(label().text(m.text.clone()).font_size(text_size(14.)).color(colors::fg_primary())),
                    );
                }
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .spacing(10.)
                    .padding(Gaps::new_all(16.))
                    .corner_radius(rounded(14.))
                    .background(colors::card())
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .cross_align(Alignment::Center)
                            .spacing(10.)
                            .child(faces_row(&who.faces, 32.))
                            .child(
                                label()
                                    .text(who.name.clone())
                                    .width(Size::flex(1.0))
                                    .font_size(text_size(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child((!who.team).then(|| {
                                Button::new().ghost().small().text("Remove friend").on_press(move |_| {
                                    let u = remove_uuid.clone();
                                    open.set(None);
                                    act(social, status, move |st| async move { chrome_service::friend_action(&st, "remove", &u).await }, None)
                                })
                            })),
                    )
                    .child(ScrollArea::new().width(Size::fill()).height(Size::flex(1.0)).child(msgs))
                    .maybe_child(who.team.then(|| muted("You can send one message here every 14 days, so make it count.")))
                    .child(
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .cross_align(Alignment::Center)
                            .child(TextInput::new(draft).placeholder("Message").width(Size::flex(1.0)).on_submit(move |_| send(())))
                            .child(Button::new().primary().text("Send").on_press(move |_| send(()))),
                    )
                    .into_element()
            }
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new(0., 40., 32., 40.))
            .spacing(16.)
            .child(
                rect()
                    .vertical()
                    .spacing(2.)
                    .child(
                        label()
                            .text("Friends")
                            .font_size(text_size(32.))
                            .font_weight(FontWeight::BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(muted(&status_text)),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .spacing(20.)
                    .child(left)
                    .child(right),
            )
    }
}

async fn refresh(mut social: State<Option<SocialState>>, mut status: State<String>) {
    let got = launcher::off_ui(async {
        let state = launcher::state().map_err(|e| e.to_string())?;
        chrome_service::social_state(&state).await
    })
    .await;
    match got {
        Ok(s) => {
            if social.peek().as_ref() != Some(&s) {
                social.set(Some(s));
            }
            if status.peek().starts_with("Connecting") || status.peek().starts_with("Can't") {
                status.set(String::new());
            }
        }
        Err(err) => status.set(err),
    }
}

/// Runs a friends action, shows its result and refreshes the list
fn act<F, Fut>(social: State<Option<SocialState>>, mut status: State<String>, work: F, done: Option<&'static str>)
where
    F: FnOnce(std::sync::Arc<scclient_core::LauncherState>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    spawn(async move {
        let result = launcher::off_ui(async move {
            let state = launcher::state().map_err(|e| e.to_string())?;
            work(state).await
        })
        .await;
        match result {
            Ok(()) => {
                status.set(done.unwrap_or_default().to_string());
                refresh(social, status).await;
            }
            Err(err) => status.set(err),
        }
    });
}

fn section(text: &str) -> Element {
    label()
        .text(text.to_string())
        .font_size(text_size(12.))
        .font_weight(FontWeight::BOLD)
        .color(colors::fg_secondary())
        .into_element()
}

fn muted(text: &str) -> Element {
    label().text(text.to_string()).font_size(text_size(13.)).color(colors::fg_secondary()).into_element()
}

fn person_row(name: &str, color: Color) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(6.)
        .cross_align(Alignment::Center)
        .child(label().text(name.to_string()).width(Size::flex(1.0)).font_size(text_size(14.)).color(color))
}

/// player heads side by side (the SC Client chat shows both creators)
fn faces_row(uuids: &[String], size: f32) -> Element {
    let mut row = rect().horizontal().spacing(2.).cross_align(Alignment::Center);
    for uuid in uuids.iter().take(3) {
        row = row.child(Avatar::new(uuid.clone()).size(size).radius(size / 4.));
    }
    row.into_element()
}

fn dot(online: bool) -> Element {
    rect()
        .width(Size::px(9.))
        .height(Size::px(9.))
        .corner_radius(rounded(5.))
        .background(if online { Color::from_rgb(46, 204, 113) } else { colors::fg_secondary() })
        .into_element()
}
