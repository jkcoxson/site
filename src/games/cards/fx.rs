// Jackson Coxson
// Card movement on the card tables (browser only).
//
// The page re-renders from each server update, so motion is done FLIP style:
// measure where things are before the update, let Leptos render, then on the
// next frame animate each card from its old spot to its new one.
//
// Pages mark up what can move:
// - `data-card="<id>"` on every face-up card, in a hand or on the table. A card
//   whose element moves (hand → discard pile, hand → trick) slides there.
// - `data-pile="<name>"` on a pile (the deck), whose first child is its face.
// - `data-seat="<player>"` on each player's seat at the top of the table.
//
// A game describes anything else about its latest move with a `Motion`.

use std::collections::HashMap;

use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{Element, HtmlElement};

use super::{Motion, Spot};

#[derive(Clone, Copy)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn of(el: &Element) -> Self {
        let r = el.get_bounding_client_rect();
        Rect {
            x: r.left(),
            y: r.top(),
            w: r.width(),
            h: r.height(),
        }
    }

    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// A small card-shaped rect in the middle of this one, as if tossed from
    /// a seat.
    fn token(&self) -> Self {
        let (cx, cy) = self.center();
        Rect {
            x: cx - 10.0,
            y: cy - 14.0,
            w: 20.0,
            h: 28.0,
        }
    }
}

/// Where everything was before an update. Card and pile faces are cloned so
/// one that leaves can still be drawn flying away after it re-renders.
pub struct Snapshot {
    cards: HashMap<u8, (Rect, Element)>,
    piles: HashMap<String, (Rect, Element)>,
    seats: HashMap<u8, Rect>,
}

fn document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

fn each(selector: &str, mut f: impl FnMut(Element)) {
    let Some(list) = document().and_then(|d| d.query_selector_all(selector).ok()) else {
        return;
    };
    for i in 0..list.length() {
        if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
            f(el);
        }
    }
}

fn id_attr(el: &Element, name: &str) -> Option<u8> {
    el.get_attribute(name)?.parse().ok()
}

fn one(selector: &str) -> Option<Element> {
    document()?.query_selector(selector).ok().flatten()
}

/// The face currently showing on a pile.
fn pile(name: &str) -> Option<Element> {
    one(&format!("[data-pile=\"{name}\"] > *"))
}

fn seat(player: u8) -> Option<Rect> {
    one(&format!("[data-seat=\"{player}\"]")).map(|el| Rect::of(&el))
}

pub fn card(id: u8) -> Option<HtmlElement> {
    one(&format!("[data-card=\"{id}\"]"))?.dyn_into().ok()
}

fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .is_some_and(|m| m.matches())
}

fn copy(el: &Element) -> Option<Element> {
    el.clone_node_with_deep(true)
        .ok()?
        .dyn_into::<Element>()
        .ok()
}

pub fn snapshot() -> Snapshot {
    let mut cards = HashMap::new();
    each("[data-card]", |el| {
        if let (Some(id), Some(c)) = (id_attr(&el, "data-card"), copy(&el)) {
            cards.insert(id, (Rect::of(&el), c));
        }
    });
    let mut piles = HashMap::new();
    each("[data-pile]", |el| {
        let Some(name) = el.get_attribute("data-pile") else {
            return;
        };
        if let Some(face) = el.first_element_child() {
            if let Some(c) = copy(&face) {
                piles.insert(name, (Rect::of(&face), c));
            }
        }
    });
    let mut seats = HashMap::new();
    each("[data-seat]", |el| {
        if let Some(id) = id_attr(&el, "data-seat") {
            seats.insert(id, Rect::of(&el));
        }
    });
    Snapshot {
        cards,
        piles,
        seats,
    }
}

fn js_fn(target: &JsValue, name: &str) -> Option<js_sys::Function> {
    js_sys::Reflect::get(target, &name.into())
        .ok()?
        .dyn_into()
        .ok()
}

/// `element.animate(frames, options)`, called through JS so no extra web-sys
/// features are needed.
fn animate(
    el: &Element,
    frames: &[&[(&str, String)]],
    ms: f64,
    delay: f64,
    easing: &str,
    fill: &str,
) {
    let list = js_sys::Array::new();
    for frame in frames {
        let obj = js_sys::Object::new();
        for (k, v) in frame.iter() {
            let _ = js_sys::Reflect::set(&obj, &(*k).into(), &v.into());
        }
        list.push(&obj);
    }
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"duration".into(), &ms.into());
    let _ = js_sys::Reflect::set(&opts, &"delay".into(), &delay.into());
    let _ = js_sys::Reflect::set(&opts, &"easing".into(), &easing.into());
    let _ = js_sys::Reflect::set(&opts, &"fill".into(), &fill.into());
    if let Some(f) = js_fn(el, "animate") {
        let _ = f.call2(el, &list, &opts);
    }
}

fn cancel_animations(el: &Element) {
    let Some(list) = js_fn(el, "getAnimations").and_then(|f| f.call0(el).ok()) else {
        return;
    };
    for anim in js_sys::Array::from(&list).iter() {
        if let Some(cancel) = js_fn(&anim, "cancel") {
            let _ = cancel.call0(&anim);
        }
    }
}

fn next_frame(f: impl FnOnce() + 'static) {
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(Closure::once_into_js(f).unchecked_ref());
    }
}

/// Run `f` after `ms` milliseconds.
pub fn after(ms: i32, f: impl FnOnce() + 'static) {
    if let Some(w) = web_sys::window() {
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
            Closure::once_into_js(f).unchecked_ref(),
            ms,
        );
    }
}

/// The transform that puts an element laid out at `to` where `from` is.
fn offset(from: Rect, to: Rect) -> String {
    let (fx, fy) = from.center();
    let (tx, ty) = to.center();
    let scale = if to.w > 0.0 { from.w / to.w } else { 1.0 };
    format!("translate({}px, {}px) scale({scale})", fx - tx, fy - ty)
}

/// Slide an element from where it was to where it now sits.
fn slide(el: &Element, from: Rect, to: Rect, ms: f64) {
    if (from.x - to.x).abs() < 1.0 && (from.y - to.y).abs() < 1.0 {
        return;
    }
    animate(
        el,
        &[
            &[("transform", offset(from, to))],
            &[("transform", "none".into())],
        ],
        ms,
        0.0,
        "cubic-bezier(.2,.8,.2,1)",
        "none",
    );
}

/// Fly a newly arrived card in from `from`, above everything else.
fn fly_in(el: &Element, from: Rect, to: Rect, delay: f64) {
    animate(
        el,
        &[
            &[
                ("transform", format!("{} rotate(-12deg)", offset(from, to))),
                ("zIndex", "50".into()),
            ],
            &[("transform", "none".into()), ("zIndex", "50".into())],
        ],
        420.0,
        delay,
        "cubic-bezier(.2,.8,.2,1)",
        "backwards",
    );
}

/// Send a copy of a card off toward a seat, or let it sink away and fade
/// when there's no seat to go to.
fn ghost(copy: Element, from: Rect, to: Option<Rect>, delay: f64) {
    let Some(body) = document().and_then(|d| d.body()) else {
        return;
    };
    let Ok(el) = copy.dyn_into::<HtmlElement>() else {
        return;
    };
    let _ = el.remove_attribute("data-card");
    let style = el.style();
    for (k, v) in [
        ("position", "fixed".to_string()),
        ("left", format!("{}px", from.x)),
        ("top", format!("{}px", from.y)),
        ("width", format!("{}px", from.w)),
        ("height", format!("{}px", from.h)),
        ("margin", "0".into()),
        ("z-index", "60".into()),
        ("pointer-events", "none".into()),
        ("translate", "none".into()),
    ] {
        let _ = style.set_property(k, &v);
    }
    if body.append_child(&el).is_err() {
        return;
    }
    let (fx, fy) = from.center();
    let (tx, ty) = match to {
        Some(to) => to.center(),
        None => (fx, fy + 60.0),
    };
    let end_scale = if to.is_some() { 0.3 } else { 0.6 };
    animate(
        &el,
        &[
            &[("transform", "none".into()), ("opacity", "1".into())],
            &[
                (
                    "transform",
                    format!(
                        "translate({}px, {}px) scale(1.1)",
                        (tx - fx) * 0.15,
                        (ty - fy) * 0.15
                    ),
                ),
                ("opacity", "1".into()),
            ],
            &[
                (
                    "transform",
                    format!("translate({}px, {}px) scale({end_scale})", tx - fx, ty - fy),
                ),
                ("opacity", "0".into()),
            ],
        ],
        650.0,
        delay,
        "ease-in",
        "both",
    );
    after(700 + delay as i32, move || el.remove());
}

/// After the next render, move every card from where `snap` saw it and play
/// `motion`. `then` runs first, once the new layout exists.
pub fn play(snap: Snapshot, motion: Motion, then: Option<Box<dyn FnOnce()>>) {
    next_frame(move || {
        if let Some(then) = then {
            then();
        }
        if reduced_motion() {
            return;
        }
        let spot = |s: &Spot| -> Option<Rect> {
            match s {
                Spot::Pile(name) => pile(name)
                    .map(|el| Rect::of(&el))
                    .or_else(|| snap.piles.get(*name).map(|p| p.0)),
                Spot::Card(id) => snap.cards.get(id).map(|c| c.0),
                Spot::Seat(p) => seat(*p)
                    .or_else(|| snap.seats.get(p).copied())
                    .map(|r| r.token()),
            }
        };
        let arrive_from = motion.arrive_from.as_ref().and_then(spot);
        let mut dealt = 0.0;
        each("[data-card]", |el| {
            let Some(id) = id_attr(&el, "data-card") else {
                return;
            };
            if el.class_list().contains("card-dragging") {
                return;
            }
            if let Some((old, _)) = snap.cards.get(&id) {
                // Only restart the motion when the card actually moved, so a
                // deal in progress isn't cut short by an unrelated update.
                let now = Rect::of(&el);
                if (old.x - now.x).abs() >= 1.0 || (old.y - now.y).abs() >= 1.0 {
                    cancel_animations(&el);
                    let now = Rect::of(&el);
                    slide(&el, *old, now, 300.0);
                }
                return;
            }
            if let Some(from) = arrive_from {
                let delay = if motion.deal {
                    dealt += 1.0;
                    (dealt - 1.0) * 70.0
                } else {
                    0.0
                };
                fly_in(&el, from, Rect::of(&el), delay);
            }
        });

        for (i, (what, to)) in motion.leave.iter().enumerate() {
            let found = match what {
                Spot::Pile(name) => snap.piles.get(*name),
                Spot::Card(id) => snap.cards.get(id),
                Spot::Seat(_) => None,
            };
            if let Some((from, copy)) = found {
                let to = to.and_then(|p| seat(p).or_else(|| snap.seats.get(&p).copied()));
                ghost(copy.clone(), *from, to, i as f64 * 60.0);
            }
        }
    });
}

// ── Dragging cards around the hand ────────────────────────────────────────────

/// Pick a card up so it follows the pointer.
pub fn lift(id: u8) {
    if let Some(el) = card(id) {
        cancel_animations(&el);
        let _ = el.class_list().add_1("card-dragging");
    }
}

/// Keep the dragged card under the pointer. `grab_dx` is how far into the
/// card the pointer went down; `dy` is the vertical drag distance.
pub fn follow(hand: &Element, id: u8, client_x: f64, grab_dx: f64, dy: f64) {
    let Some(el) = card(id) else {
        return;
    };
    let left = hand.get_bounding_client_rect().left();
    let tx = client_x - left - grab_dx - el.offset_left() as f64;
    let ty = dy.clamp(-70.0, 30.0);
    let _ = el
        .style()
        .set_property("transform", &format!("translate({tx}px, {ty}px)"));
}

/// Where the dragged card would land: its index among the hand's cards.
pub fn drop_index(hand: &Element, id: u8, client_x: f64, grab_dx: f64) -> Option<usize> {
    let dragged = card(id)?;
    let left = hand.get_bounding_client_rect().left();
    let middle = client_x - left - grab_dx + dragged.offset_width() as f64 / 2.0;
    let mut index = 0;
    let list = hand.query_selector_all("[data-card]").ok()?;
    for i in 0..list.length() {
        let Some(el) = list.item(i).and_then(|n| n.dyn_into::<HtmlElement>().ok()) else {
            continue;
        };
        if id_attr(&el, "data-card") == Some(id) {
            continue;
        }
        if (el.offset_left() as f64 + el.offset_width() as f64 / 2.0) < middle {
            index += 1;
        }
    }
    Some(index)
}

/// Let go of the dragged card and let it settle into its slot.
pub fn settle(id: u8) {
    let Some(el) = card(id) else {
        return;
    };
    let from = Rect::of(&el);
    let _ = el.style().remove_property("transform");
    let _ = el.class_list().remove_1("card-dragging");
    let to = Rect::of(&el);
    slide(&el, from, to, 180.0);
}
