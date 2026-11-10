// Jackson Coxson
// Age of Monuments: lead an ancient city through three ages, drafting a
// card each turn to build up the city, trade with the neighbors and raise
// its monument. Built on the shared card table, which supplies the lobby,
// rooms, seats and card motion.
//
// Cards carry a lot of detail, so a hand card shows its name, cost and
// effect in icons and tapping it opens the full text. Every city is public:
// the tabs under the hand step around the table to any player's city.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, card_overlay, final_standings, peek_tabs, seat_strip, table_talk, GameKind, PeekTab,
    SeatChip, TAG_DARK, TAG_GREEN, TAG_SOFT,
};
use crate::games::rules::{RulesButton, RulesSection};
use logic::{
    age_name, age_of, build_cost, can_use_power, def, icons, leads_to, mask_res, pass_dir, score,
    stage_cost, Act, Breakdown, CardDef, Choice, City, ClientMsg, Color, Cost, Discount, Done,
    Effect as Eff, GameView, Gift, League, Options, Pay, Per, Res, Sci, Sides, Stage, TURNS,
};

fn send_msg(msg: &ClientMsg) {
    cards::send(msg);
}

// ── Looks ─────────────────────────────────────────────────────────────────────

/// Classes for a card's name band.
fn band(color: Color) -> &'static str {
    match color {
        Color::Brown => "bg-amber-800 text-amber-50",
        Color::Gray => "bg-slate-500 text-white",
        Color::Blue => "bg-blue-600 text-white",
        Color::Yellow => "bg-yellow-400 text-stone-900",
        Color::Red => "bg-red-600 text-white",
        Color::Green => "bg-emerald-600 text-white",
        Color::Purple => "bg-purple-700 text-white",
    }
}

/// A small colored square standing for cards of a color.
fn swatch(color: Color) -> impl IntoView {
    let class = match color {
        Color::Brown => "mon-sq bg-amber-800",
        Color::Gray => "mon-sq bg-slate-500",
        Color::Blue => "mon-sq bg-blue-600",
        Color::Yellow => "mon-sq bg-yellow-400",
        Color::Red => "mon-sq bg-red-600",
        Color::Green => "mon-sq bg-emerald-600",
        Color::Purple => "mon-sq bg-purple-700",
    };
    view! { <span class=class title=color.plural()></span> }
}

fn repeat(s: &str, n: u8) -> String {
    (0..n).map(|_| s).collect()
}

fn choice_icons(res: &[Res]) -> String {
    res.iter().map(|r| r.icon()).collect::<Vec<_>>().join("/")
}

const ANY_SCI: &str = "🔭/⚙️/🪶";

/// A card's effect in icons, and whether it's short enough to show large.
fn effect_face(e: Eff) -> (AnyView, bool) {
    let text = |s: String| view! { <span>{s}</span> }.into_any();
    match e {
        Eff::Make(res, n) if res.len() == 1 => (text(repeat(res[0].icon(), n)), true),
        Eff::Make(res, _) => (text(choice_icons(res)), res.len() <= 2),
        Eff::Points(p) => (text(format!("⭐{p}")), true),
        Eff::Shields(n) => (text(repeat("🛡️", n)), true),
        Eff::Science(s) => (text(s.icon().into()), true),
        Eff::Coins(c) => (text(format!("🟡{c}")), true),
        Eff::Discount(Discount::RawLeft) => (text("◂ 🪵🪨🧱⛏️ 1🟡".into()), false),
        Eff::Discount(Discount::RawRight) => (text("🪵🪨🧱⛏️ 1🟡 ▸".into()), false),
        Eff::Discount(Discount::Goods) => (text("◂ 🫙🧵📜 1🟡 ▸".into()), false),
        Eff::CoinsAround(color, k) => (
            view! { <span>{format!("🟡{k} per ")} {swatch(color)} " ◂•▸"</span> }.into_any(),
            false,
        ),
        Eff::Bounty(Per::Cards(color), k, p) => (
            view! { <span>{format!("🟡{k} ⭐{p} per ")} {swatch(color)}</span> }.into_any(),
            false,
        ),
        Eff::Bounty(Per::Stages, k, p) => (text(format!("🟡{k} ⭐{p} per 🏛️")), false),
        Eff::League(League::Neighbors(color, k)) => (
            view! { <span>{format!("⭐{k} per ")} {swatch(color)} " ◂▸"</span> }.into_any(),
            false,
        ),
        Eff::League(League::Defeats) => (text("⭐1 per ◂ −1 ▸".into()), false),
        Eff::League(League::Shipwrights) => (
            view! {
                <span>
                    "⭐1 per " {swatch(Color::Brown)} {swatch(Color::Gray)} {swatch(Color::Purple)}
                </span>
            }
            .into_any(),
            false,
        ),
        Eff::League(League::Scholars) => (text(ANY_SCI.into()), false),
        Eff::League(League::Masons) => (text("⭐1 per 🏛️ ◂•▸".into()), false),
    }
}

/// A card's effect, spelled out.
fn effect_text(e: Eff) -> String {
    let names = |res: &[Res]| {
        res.iter()
            .map(|r| format!("{} {}", r.icon(), r.name()))
            .collect::<Vec<_>>()
            .join(" or ")
    };
    match e {
        Eff::Make(res, 1) if res.len() == 1 => format!("Makes one {} every turn.", names(res)),
        Eff::Make(res, n) if res.len() == 1 => format!("Makes {n} {} every turn.", names(res)),
        Eff::Make(res, _) => format!("Makes one {} every turn, your pick each time.", names(res)),
        Eff::Points(p) => format!("Worth {p} points at the end."),
        Eff::Shields(1) => "1 shield for the wars at the end of each age.".into(),
        Eff::Shields(n) => format!("{n} shields for the wars at the end of each age."),
        Eff::Science(s) => format!("A {} {} symbol for science.", s.icon(), s.name()),
        Eff::Coins(c) => format!("Take {c} coins from the bank when you build it."),
        Eff::Discount(Discount::RawLeft) => {
            "Raw materials (🪵🪨🧱⛏️) from your left neighbor cost you 1 coin instead of 2.".into()
        }
        Eff::Discount(Discount::RawRight) => {
            "Raw materials (🪵🪨🧱⛏️) from your right neighbor cost you 1 coin instead of 2.".into()
        }
        Eff::Discount(Discount::Goods) => {
            "Crafted goods (🫙🧵📜) from either neighbor cost you 1 coin instead of 2.".into()
        }
        Eff::CoinsAround(color, k) => format!(
            "When built, take {k} coin{} per {} in your city and both neighbors' (counting ones built this turn).",
            if k == 1 { "" } else { "s" },
            &color.plural()[..color.plural().len() - 1]
        ),
        Eff::Bounty(Per::Cards(color), k, p) => format!(
            "When built, take {k} coin{} per {} in your city. At the end, {p} point{} per {} in your city.",
            if k == 1 { "" } else { "s" },
            &color.plural()[..color.plural().len() - 1],
            if p == 1 { "" } else { "s" },
            &color.plural()[..color.plural().len() - 1],
        ),
        Eff::Bounty(Per::Stages, k, p) => format!(
            "When built, take {k} coins per monument stage you've built. At the end, {p} point per stage."
        ),
        Eff::League(League::Neighbors(color, k)) => format!(
            "At the end, {k} point{} per {} in your two neighbors' cities.",
            if k == 1 { "" } else { "s" },
            &color.plural()[..color.plural().len() - 1]
        ),
        Eff::League(League::Defeats) => {
            "At the end, 1 point per defeat token your two neighbors hold.".into()
        }
        Eff::League(League::Shipwrights) => {
            "At the end, 1 point per brown, gray and purple card in your own city (this one too).".into()
        }
        Eff::League(League::Scholars) => {
            format!("At the end, counts as the science symbol of your choice ({ANY_SCI}).")
        }
        Eff::League(League::Masons) => {
            "At the end, 1 point per monument stage built in your city and both neighbors'.".into()
        }
    }
}

fn gift_face(g: Gift) -> String {
    match g {
        Gift::Points(p) => format!("⭐{p}"),
        Gift::Coins(c) => format!("🟡{c}"),
        Gift::Shields(n) => repeat("🛡️", n),
        Gift::AnyRaw => choice_icons(&Res::RAW),
        Gift::AnyGoods => choice_icons(&Res::GOODS),
        Gift::Science => ANY_SCI.into(),
        Gift::FreeBuild => "🆓 build / age".into(),
        Gift::Salvage => "♻️ discards".into(),
        Gift::Seventh => "7th card".into(),
        Gift::CheapRaw => "◂ raw 1🟡 ▸".into(),
        Gift::CopyLeague => "copy ◂🟪▸".into(),
    }
}

fn gift_text(g: Gift) -> String {
    match g {
        Gift::Points(p) => format!("{p} points at the end"),
        Gift::Coins(c) => format!("{c} coins from the bank, once"),
        Gift::Shields(n) => format!("{n} shield{} for the wars", if n == 1 { "" } else { "s" }),
        Gift::AnyRaw => "one raw material of your choice every turn (not for sale)".into(),
        Gift::AnyGoods => "one crafted good of your choice every turn (not for sale)".into(),
        Gift::Science => {
            "an extra science symbol (🔭, ⚙️ or 🪶) at the end, whichever scores best".into()
        }
        Gift::FreeBuild => "once per age, build a card from your hand for free".into(),
        Gift::Salvage => {
            "right after building it, look through every discarded card and build one for free"
                .into()
        }
        Gift::Seventh => {
            "from then on, play the last card of each age instead of discarding it".into()
        }
        Gift::CheapRaw => "raw materials from both neighbors cost 1 coin".into(),
        Gift::CopyLeague => {
            "at the end, the points of one purple league a neighbor built (the best one is picked for you)"
                .into()
        }
    }
}

/// Everything a stage gives, as one phrase.
fn stage_text(gifts: &[Gift]) -> String {
    let parts: Vec<String> = gifts.iter().map(|&g| gift_text(g)).collect();
    match parts.len() {
        0 => "nothing".into(),
        1 => parts[0].clone(),
        n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// A card's cost in icons ("" when free).
fn cost_icons(d: &CardDef) -> String {
    let mut s = icons(d.cost);
    if d.coins > 0 {
        s.push_str(&format!("🟡{}", d.coins));
    }
    s
}

/// A card at whatever size its slot gives it; text scales with the card.
fn card_face(id: u8) -> impl IntoView {
    let d = def(id);
    let (effect, big) = effect_face(d.effect);
    let cost = cost_icons(d);
    view! {
        <div class="mon-slot">
            <div class="mon-card">
                <div class=format!("mon-band {}", band(d.color))>{d.name}</div>
                <div class="mon-cost">
                    {if cost.is_empty() { "free".to_string() } else { cost }}
                </div>
                <div class=if big {
                    "mon-effect"
                } else {
                    "mon-effect mon-effect-sm"
                }>{effect}</div>
                {(!d.free_with.is_empty())
                    .then(|| {
                        view! {
                            <div class="mon-foot">
                                {format!("🔗 {}", d.free_with.join(" / "))}
                            </div>
                        }
                    })}
            </div>
        </div>
    }
}

fn card_back() -> impl IntoView {
    view! {
        <div class="flex h-12 w-9 select-none items-center justify-center rounded-md bg-gradient-to-br from-amber-700 to-stone-900 text-lg shadow ring-1 ring-amber-500">
            "🏛️"
        </div>
    }
}

/// Everything about a card, for the zoomed view and the pick panel.
fn card_detail(id: u8) -> impl IntoView {
    let d = def(id);
    let later = leads_to(d);
    let cost = cost_icons(d);
    view! {
        <div class="space-y-2 text-sm">
            <div class="flex items-start gap-3">
                <div class="w-24 shrink-0">{card_face(id)}</div>
                <div class="min-w-0 space-y-1">
                    <p class="text-lg font-bold leading-tight">{d.name}</p>
                    <p class="flex items-center gap-1 text-xs opacity-75">
                        {swatch(d.color)}
                        {format!("{} · Age {}", d.color.kind(), age_name(age_of(id)))}
                    </p>
                    <p>
                        <span class="opacity-75">"Cost: "</span>
                        {if cost.is_empty() { "free".to_string() } else { cost }}
                    </p>
                </div>
            </div>
            <p>{effect_text(d.effect)}</p>
            {(!d.free_with.is_empty())
                .then(|| {
                    view! {
                        <p class="text-xs opacity-80">
                            {format!(
                                "🔗 Free if you already built {}.",
                                d.free_with.join(" or "),
                            )}
                        </p>
                    }
                })}
            {(!later.is_empty())
                .then(|| {
                    view! {
                        <p class="text-xs opacity-80">
                            {format!("➜ Later builds {} for free.", later.join(" and "))}
                        </p>
                    }
                })}
        </div>
    }
}

fn pay_text(g: &GameView, pay: &Pay) -> String {
    let mut parts = Vec::new();
    if pay.left > 0 {
        parts.push(format!("{}🟡 to ◂ {}", pay.left, g.name(g.left())));
    }
    if pay.right > 0 {
        parts.push(format!("{}🟡 to {} ▸", pay.right, g.name(g.right())));
    }
    if pay.bank > 0 {
        parts.push(format!("{}🟡 to the bank", pay.bank));
    }
    if parts.is_empty() {
        "nothing to pay".into()
    } else {
        format!("pay {}", parts.join(", "))
    }
}

fn done_text(g: &GameView, done: Done, pay: &Pay) -> String {
    let what = match done {
        Done::Built(id) => format!("Built {}", def(id).name),
        Done::Free(id) => format!("Built {} free", def(id).name),
        Done::Stage(k) => format!("Raised monument stage {k}"),
        Done::Sold => "Sold a card for 3🟡".into(),
        Done::Salvaged(id) => format!("Built {} from the discards", def(id).name),
    };
    if pay.total() > 0 {
        format!("{what} ({})", pay_text(g, pay).trim_start_matches("pay "))
    } else {
        what
    }
}

// ── Chips ─────────────────────────────────────────────────────────────────────

const PILL: &str = "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-sm font-bold ring-1";
const PILL_COINS: &str = "bg-yellow-200 text-yellow-950 ring-yellow-500";
const PILL_SHIELDS: &str = "bg-red-100 text-red-900 ring-red-300";
const PILL_POINTS: &str = "bg-blue-100 text-blue-900 ring-blue-300";
const PILL_SCIENCE: &str = "bg-emerald-100 text-emerald-900 ring-emerald-300";

fn pill(class: &'static str, title: &'static str, body: impl IntoView) -> impl IntoView {
    view! {
        <span class=format!("{PILL} {class}") title=title>
            {body}
        </span>
    }
}

/// A coin amount going to someone: "◂ Ann 🟡2", "Bob ▸ 🟡1" or "🏦 🟡1".
fn coin_chip(who: String, n: u8) -> impl IntoView {
    view! {
        <span class="inline-flex items-center gap-1 rounded-md bg-yellow-200 px-1.5 py-0.5 text-xs font-bold text-yellow-950 ring-1 ring-yellow-500">
            <span class="max-w-[7rem] truncate font-semibold">{who}</span>
            {format!("🟡{n}")}
        </span>
    }
}

/// Who gets paid what, as chips; nothing when it's free.
fn pay_chips(g: &GameView, pay: &Pay) -> AnyView {
    let mut chips = Vec::new();
    if pay.left > 0 {
        chips.push(coin_chip(format!("◂ {}", g.name(g.left())), pay.left).into_any());
    }
    if pay.right > 0 {
        chips.push(coin_chip(format!("{} ▸", g.name(g.right())), pay.right).into_any());
    }
    if pay.bank > 0 {
        chips.push(coin_chip("🏦".into(), pay.bank).into_any());
    }
    view! { <span class="flex flex-wrap gap-1">{chips}</span> }.into_any()
}

/// A short badge for a hand card's cheapest price: "◂🟡2 🟡1▸".
fn pay_badge(pay: &Pay) -> String {
    let mut parts = Vec::new();
    if pay.left > 0 {
        parts.push(format!("◂🟡{}", pay.left));
    }
    if pay.bank > 0 {
        parts.push(format!("🟡{}", pay.bank));
    }
    if pay.right > 0 {
        parts.push(format!("🟡{}▸", pay.right));
    }
    parts.join(" ")
}

/// What a city makes each turn, grouped: (resources it can be, how many,
/// whether neighbors can buy it). Single resources first.
fn production(city: &City) -> Vec<(u8, u8, bool)> {
    let sale = city.producers(true);
    let mut all = city.producers(false);
    let mut out: Vec<(u8, u8, bool)> = Vec::new();
    let mut add = |m: u8, for_sale: bool| {
        if let Some(e) = out.iter_mut().find(|e| e.0 == m && e.2 == for_sale) {
            e.1 += 1;
        } else {
            out.push((m, 1, for_sale));
        }
    };
    for m in sale {
        if let Some(i) = all.iter().position(|&x| x == m) {
            all.remove(i);
        }
        add(m, true);
    }
    for m in all {
        add(m, false);
    }
    out.sort_by_key(|&(m, _, sale)| (!sale, m.count_ones() > 1, m.trailing_zeros()));
    out
}

/// A city's resources as chips. Raw materials are brown like their cards,
/// crafted goods gray. With `buyer`, the chips show what one unit costs
/// that neighbor (`Some(true)` when the city sits on the buyer's left);
/// what can't be bought is dashed.
fn resource_chips(city: &City, buyer: Option<(&City, bool)>) -> AnyView {
    let chips = production(city);
    if chips.is_empty() {
        return view! { <span class="text-xs opacity-60">"nothing"</span> }.into_any();
    }
    chips
        .into_iter()
        .map(|(m, n, for_sale)| {
            let res = mask_res(m);
            let raw = res.first().is_some_and(|r| r.is_raw());
            let mut class = String::from(
                "inline-flex items-center gap-0.5 rounded-md px-1.5 py-0.5 text-sm leading-tight ",
            );
            class.push_str(if raw {
                "bg-amber-100 text-amber-950 dark:bg-amber-900/60 dark:text-amber-50"
            } else {
                "bg-slate-200 text-slate-900 dark:bg-slate-600 dark:text-white"
            });
            class.push_str(match (for_sale, raw) {
                (true, true) => " ring-1 ring-amber-700/50",
                (true, false) => " ring-1 ring-slate-500/50",
                (false, _) => " border border-dashed border-stone-500 opacity-70",
            });
            let title = if for_sale {
                "Neighbors can buy this"
            } else {
                "Only for this city's own use"
            };
            let price = buyer.filter(|_| for_sale).and_then(|(me, left)| {
                res.first().map(|&r| me.price(r, left))
            });
            view! {
                <span class=class title=title>
                    <span>{choice_icons(&res)}</span>
                    {(n > 1)
                        .then(|| {
                            view! { <span class="text-xs font-bold">{format!("×{n}")}</span> }
                        })}
                    {price
                        .map(|p| {
                            let cheap = p == 1;
                            view! {
                                <span class=if cheap {
                                    "ml-0.5 rounded bg-green-600 px-1 text-[0.65rem] font-bold text-white"
                                } else {
                                    "ml-0.5 rounded bg-yellow-300 px-1 text-[0.65rem] font-bold text-yellow-950"
                                }>{format!("{p}🟡")}</span>
                            }
                        })}
                </span>
            }
        })
        .collect_view()
        .into_any()
}

/// Your two neighbors and you side by side: what each makes and what
/// buying from them costs you.
fn trade_strip(g: &GameView) -> AnyView {
    let cities = g.cities();
    let me = &cities[g.you as usize];
    let row = |label: String, coins: i32, city: &City, buyer: Option<(&City, bool)>, you: bool| {
        view! {
            <div class=if you {
                "space-y-1 rounded-lg bg-white p-2 ring-2 ring-blue-400 dark:bg-stone-800"
            } else {
                "space-y-1 rounded-lg bg-white p-2 ring-1 ring-stone-300 dark:bg-stone-800 dark:ring-stone-600"
            }>
                <div class="flex items-center justify-between gap-1 text-xs font-semibold">
                    <span class="truncate">{label}</span>
                    <span class="shrink-0 rounded-full bg-yellow-200 px-1.5 text-yellow-950">
                        {format!("🟡{coins}")}
                    </span>
                </div>
                <div class="flex flex-wrap gap-1">{resource_chips(city, buyer)}</div>
            </div>
        }
    };
    let left = &cities[g.left() as usize];
    let right = &cities[g.right() as usize];
    view! {
        <div class="grid gap-1.5 sm:grid-cols-3 dark:text-stone-100">
            {row(format!("◂ {}", g.name(g.left())), left.coins, left, Some((me, true)), false)}
            {row("You make".into(), me.coins, me, None, true)}
            {row(
                format!("{} ▸", g.name(g.right())),
                right.coins,
                right,
                Some((me, false)),
                false,
            )}
        </div>
    }
    .into_any()
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play Age of Monuments">
            <MonumentsRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn MonumentsRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>
                "You lead an ancient city with a monument to raise. Over three ages you'll \
                build up the city, trade, fight your neighbors and study the sciences. The \
                most points after Age III wins; ties go to the most coins."
            </p>
        </RulesSection>
        <RulesSection heading="A turn">
            <p>
                "Each age deals everyone seven cards. Every turn, all players pick one card \
                at the same time, then everyone plays theirs at once. With your card you can:"
            </p>
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"Build it"</span>
                    " by paying its cost. You can't build two of the same building."
                </li>
                <li>
                    <span class=b>"Raise your monument"</span>
                    ": tuck the card under it (face down) and pay the next stage's cost instead. \
                    Stages go in order and give their reward at once."
                </li>
                <li>
                    <span class=b>"Sell it"</span>
                    " for 3🟡. You can always do this."
                </li>
            </ul>
            <p>
                "Then you pass the rest of your hand on: to the left in Ages I and III, to \
                the right in Age II. After six turns the seventh card is discarded."
            </p>
        </RulesSection>
        <RulesSection heading="Paying">
            <p>
                "Resources aren't used up: a city that makes 🪵 has it every turn. Your city \
                makes its own resource plus whatever its brown and gray cards make. A card \
                that shows "<span class=b>"🔗"</span>" is free if you already built the named \
                building in an earlier age."
            </p>
            <p>
                "Missing something? Buy it from a neighbor for 2🟡 each, paid to them. They \
                can't refuse, and selling doesn't stop them using it. You can only buy what \
                their city and brown and gray cards make, and only with coins you had at the \
                start of the turn. The game works out the cheapest ways to pay for you."
            </p>
        </RulesSection>
        <RulesSection heading="The cards">
            <ul class="space-y-1">
                <li>
                    {swatch(Color::Brown)}" "{swatch(Color::Gray)}
                    " Raw materials and crafted goods."
                </li>
                <li>{swatch(Color::Blue)}" Civic buildings: points."</li>
                <li>
                    {swatch(Color::Yellow)}
                    " Commerce: coins, cheaper trade, and resources only you can use."
                </li>
                <li>{swatch(Color::Red)}" Military: 🛡️ shields."</li>
                <li>{swatch(Color::Green)}" Science: 🔭 ⚙️ 🪶 symbols."</li>
                <li>
                    {swatch(Color::Purple)}
                    " Leagues (Age III only): points for what you and your neighbors built."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="War">
            <p>
                "At the end of each age, compare your 🛡️ with each neighbor. Beat one and \
                take a victory worth 1, 3 or 5 points (Ages I, II, III); lose and take a −1. \
                A tie gives nothing."
            </p>
        </RulesSection>
        <RulesSection heading="Scoring">
            <ul class="list-disc space-y-1 pl-5">
                <li>"War tokens, plus 1 point per 3🟡."</li>
                <li>"Monument stages, civic buildings, commerce and leagues as printed."</li>
                <li>
                    "Science: each symbol type scores its count squared, plus 7 for every set of \
                    all three. Three 🔭, two ⚙️ and one 🪶 score 9 + 4 + 1 + 7 = 21."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="Peeking">
            <p>
                "Every city is public. Use the tabs under your hand to look at any city, \
                and tap a card anywhere to read it in full. Keep an eye on your neighbors: \
                you trade with them, fight them, and sometimes sell a card just to keep it \
                out of their hands."
            </p>
        </RulesSection>
    }
}

fn options_form() -> impl IntoView {
    let options = RwSignal::new(Options::default());
    let choices = [
        (Sides::A, "Side A monuments (simpler, good to start)"),
        (Sides::B, "Side B monuments (more special powers)"),
        (Sides::Random, "Random side for each city"),
    ];
    view! {
        <div class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-600">
            <select
                class="w-full rounded border border-stone-300 bg-white px-2 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                on:change=move |ev| {
                    let key = event_target_value(&ev);
                    let sides = match key.as_str() {
                        "B" => Sides::B,
                        "Random" => Sides::Random,
                        _ => Sides::A,
                    };
                    options.update(|o| o.sides = sides);
                }
            >
                {choices
                    .into_iter()
                    .map(|(s, label)| view! { <option value=format!("{s:?}")>{label}</option> })
                    .collect_view()}
            </select>
            <p class="text-xs text-stone-500 dark:text-stone-400">"3 to 7 players."</p>
            <input
                type="hidden"
                name="options"
                prop:value=move || serde_json::to_string(&options.get()).unwrap_or_default()
            />
        </div>
    }
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StageKind {
    Choosing,
    Seventh,
    Salvage,
    Over,
}

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let stage = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Choosing => StageKind::Choosing,
            Stage::Seventh { .. } => StageKind::Seventh,
            Stage::Salvage { .. } => StageKind::Salvage,
            Stage::Over { .. } => StageKind::Over,
        })
    });
    let inspect = RwSignal::new(None::<u8>);
    let viewing = RwSignal::new(game.with_untracked(|g| g.you));

    let seats = move || {
        game.with(|g| {
            let cities = g.cities();
            g.players
                .iter()
                .map(|p| {
                    let mut tags = vec![
                        (format!("🟡{}", p.city.coins), TAG_SOFT),
                        (format!("🛡️{}", p.city.shields()), TAG_SOFT),
                    ];
                    if p.id == g.left() && p.id != g.you {
                        tags.insert(0, ("◂".into(), TAG_DARK));
                    }
                    if p.id == g.right() && p.id != g.you {
                        tags.insert(0, ("▸".into(), TAG_DARK));
                    }
                    let active = match &g.stage {
                        Stage::Choosing => {
                            if p.ready {
                                tags.push(("✓".into(), TAG_GREEN));
                            }
                            !p.ready
                        }
                        Stage::Seventh { player } | Stage::Salvage { player } => *player == p.id,
                        Stage::Over { .. } => false,
                    };
                    SeatChip {
                        id: p.id,
                        name: p.nickname.clone(),
                        you: p.id == g.you,
                        connected: p.connected,
                        active,
                        score: Some(score(&cities, p.id as usize).total),
                        tags,
                    }
                })
                .collect::<Vec<_>>()
        })
    };

    let status = move || {
        game.with(|g| match &g.stage {
            Stage::Choosing => match g.choice {
                None => "Pick a card to play.".to_string(),
                Some(_) => {
                    let waiting: Vec<String> = g
                        .players
                        .iter()
                        .filter(|p| !p.ready)
                        .map(|p| p.nickname.clone())
                        .collect();
                    format!("Waiting on {}.", waiting.join(", "))
                }
            },
            Stage::Seventh { player } if *player == g.you => {
                "Your monument lets you play your last card too.".into()
            }
            Stage::Seventh { player } => {
                format!("{} is playing their last card...", g.name(*player))
            }
            Stage::Salvage { player } if *player == g.you => {
                "Pick a card from the discards to build for free.".into()
            }
            Stage::Salvage { player } => {
                format!("{} is searching the discards...", g.name(*player))
            }
            Stage::Over { .. } => "Game over.".into(),
        })
    };
    let my_move = Memo::new(move |_| {
        game.with(|g| match &g.stage {
            Stage::Choosing => g.choice.is_none(),
            Stage::Seventh { player } | Stage::Salvage { player } => *player == g.you,
            Stage::Over { .. } => false,
        })
    });

    let tabs = move || {
        game.with(|g| {
            g.players
                .iter()
                .map(|p| PeekTab {
                    id: p.id,
                    label: if p.id == g.you {
                        "You".into()
                    } else {
                        p.nickname.clone()
                    },
                    note: if p.id == g.you {
                        p.city.board().city.to_string()
                    } else if p.id == g.left() {
                        "◂ left".into()
                    } else if p.id == g.right() {
                        "right ▸".into()
                    } else {
                        p.city.board().city.to_string()
                    },
                })
                .collect::<Vec<_>>()
        })
    };

    view! {
        <div class="grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <div data-pile="deck" class="shrink-0">
                        {card_back()}
                    </div>
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || game.with(|g| format!("Age {}", age_name(g.age)))}
                    </span>
                    {move || {
                        (stage.get() != StageKind::Over)
                            .then(|| {
                                view! {
                                    <span class="rounded bg-stone-200 px-3 py-1 dark:bg-stone-700 dark:text-white">
                                        {move || game.with(|g| format!("Turn {}/{TURNS}", g.turn))}
                                    </span>
                                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                                        {move || {
                                            game.with(|g| {
                                                let dir = pass_dir(g.age);
                                                let to = dir.to(g.players.len().max(1), g.you as usize)
                                                    as u8;
                                                match dir {
                                                    cards::seating::Pass::Left => {
                                                        format!("Pass ◂ to {}", g.name(to))
                                                    }
                                                    cards::seating::Pass::Right => {
                                                        format!("Pass to {} ▸", g.name(to))
                                                    }
                                                }
                                            })
                                        }}
                                    </span>
                                }
                            })
                    }}
                </div>

                {seat_strip(seats)}

                <p class=move || {
                    if my_move.get() {
                        "min-h-5 text-sm font-semibold text-amber-600 dark:text-amber-300"
                    } else {
                        "min-h-5 text-sm text-stone-600 dark:text-stone-300"
                    }
                }>{status}</p>

                {move || match stage.get() {
                    StageKind::Choosing | StageKind::Seventh => {
                        view! {
                            {move || game.with(trade_strip)}
                            <HandPanel game inspect />
                        }
                            .into_any()
                    }
                    StageKind::Salvage => view! { <SalvagePanel game inspect /> }.into_any(),
                    StageKind::Over => {
                        let over = move || {
                            let g = game.get();
                            let Stage::Over { scores, standings } = g.stage.clone() else {
                                return None;
                            };
                            let names = g
                                .players
                                .iter()
                                .map(|p| (p.id, p.nickname.clone()))
                                .collect();
                            Some(
                                view! {
                                    <div class="space-y-4">
                                        {final_standings(
                                            g.you,
                                            standings,
                                            names,
                                            || send_msg(&ClientMsg::PlayAgain),
                                        )} {score_table(&g, scores)}
                                    </div>
                                },
                            )
                        };
                        view! { {over} }.into_any()
                    }
                }}

                <div class="space-y-2 rounded-xl bg-stone-100 p-2 ring-1 ring-stone-200 sm:p-3 dark:bg-stone-900/60 dark:ring-stone-700">
                    {peek_tabs(tabs, viewing)}
                    {move || {
                        let id = viewing.get();
                        game.with(|g| city_panel(g, id, inspect))
                    }}
                </div>
            </div>

            <div class="space-y-3 lg:self-start">
                {table_talk(move || game.with(|g| g.log.clone()))}
            </div>
        </div>
        {card_overlay(inspect, |id| card_detail(id).into_any())}
    }
}

/// The player's hand as a grid of cards, and what to do with the one picked.
#[component]
fn HandPanel(game: Memo<GameView>, inspect: RwSignal<Option<u8>>) -> impl IntoView {
    let selected = RwSignal::new(None::<u8>);
    let hand = Memo::new(move |_| game.with(|g| g.hand.clone()));
    let locked = Memo::new(move |_| game.with(|g| g.choice));
    let can_pick = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Choosing => g.choice.is_none(),
            Stage::Seventh { player } => player == g.you,
            _ => false,
        })
    });
    // Forget a pick that's no longer in hand.
    Effect::new(move |_| {
        let h = hand.get();
        if selected.get_untracked().is_some_and(|id| !h.contains(&id)) {
            selected.set(None);
        }
    });

    // A quick hint on each card: free, what it costs in coins and to whom,
    // can't build, already have one.
    let hint = move |id: u8| -> Option<(String, &'static str)> {
        game.with(|g| {
            let cities = g.cities();
            match build_cost(&cities, g.you as usize, id) {
                Cost::Free => Some(("✓".into(), "bg-green-600 text-white")),
                Cost::Chain(_) => Some(("🔗 free".into(), "bg-green-600 text-white")),
                Cost::Pay(ways) => {
                    let cheapest = ways.iter().min_by_key(|p| p.total())?;
                    if cheapest.total() == 0 {
                        Some(("✓".into(), "bg-green-600 text-white"))
                    } else {
                        Some((pay_badge(cheapest), "bg-yellow-300 text-yellow-950"))
                    }
                }
                Cost::Cannot("You already have one.") => {
                    Some(("have it".into(), "bg-stone-600 text-white"))
                }
                Cost::Cannot(_) => Some(("✗ can't".into(), "bg-red-700 text-white")),
            }
        })
    };

    view! {
        <div class="space-y-3">
            <div class="grid grid-cols-4 gap-1.5 sm:grid-cols-7 sm:gap-2">
                <For
                    each=move || hand.get()
                    key=|id| *id
                    children=move |id| {
                        view! {
                            <button
                                data-card=id
                                class=move || {
                                    let picked = locked.get().map(|c| c.card);
                                    let mut c = String::from(
                                        "mon-hand-card relative touch-manipulation rounded-lg text-left",
                                    );
                                    if selected.get() == Some(id) || picked == Some(id) {
                                        c.push_str(" mon-pick");
                                    }
                                    if picked.is_some_and(|p| p != id) {
                                        c.push_str(" mon-dim");
                                    }
                                    c
                                }
                                on:click=move |_| {
                                    if can_pick.get_untracked() {
                                        selected
                                            .update(|s| {
                                                *s = if *s == Some(id) { None } else { Some(id) };
                                            });
                                    } else {
                                        inspect.set(Some(id));
                                    }
                                }
                            >
                                {card_face(id)}
                                {move || {
                                    hint(id)
                                        .map(|(text, class)| {
                                            view! {
                                                <span class=format!(
                                                    "absolute inset-x-1 bottom-1 rounded px-0.5 text-center text-[0.7rem] font-bold leading-tight {class}",
                                                )>{text}</span>
                                            }
                                        })
                                }}
                            </button>
                        }
                    }
                />
            </div>
            {move || {
                if let Some(c) = locked.get() {
                    return game
                        .with(|g| {
                            let what = match c.act {
                                Act::Build | Act::Power => format!("🔨 {}", def(c.card).name),
                                Act::Stage => "🏛️ Monument stage".to_string(),
                                Act::Sell => format!("💰 Sell {} for 🟡3", def(c.card).name),
                            };
                            view! {
                                <div class="flex items-center justify-between gap-2 rounded-lg border border-stone-200 bg-white p-3 text-sm dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100">
                                    <span class="flex flex-wrap items-center gap-1.5">
                                        <span class="font-semibold">{what}</span>
                                        {pay_chips(g, &c.pay)}
                                    </span>
                                    <button
                                        class="shrink-0 touch-manipulation rounded bg-stone-600 px-3 py-2 text-white hover:bg-stone-500"
                                        on:click=|_| send_msg(&ClientMsg::Unchoose)
                                    >
                                        "Change"
                                    </button>
                                </div>
                            }
                        })
                        .into_any();
                }
                if !can_pick.get() {
                    return view! {
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            "Tap a card to read it."
                        </p>
                    }
                        .into_any();
                }
                match selected.get() {
                    None => {
                        view! {
                            <p class="text-xs text-stone-500 dark:text-stone-400">
                                "Tap a card to see what you can do with it."
                            </p>
                        }
                            .into_any()
                    }
                    Some(id) => game.with(|g| actions(g, id)).into_any(),
                }
            }}
        </div>
    }
}

const BTN: &str = "w-full touch-manipulation rounded-lg px-3 py-2.5 text-left text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-50";
const BTN_GO: &str = "bg-blue-500 text-white hover:bg-blue-600";
const BTN_ALT: &str = "bg-amber-500 text-stone-900 hover:bg-amber-400";
const BTN_SELL: &str = "bg-stone-600 text-white hover:bg-stone-500";

fn choose(card: u8, act: Act, pay: Pay) {
    send_msg(&ClientMsg::Choose {
        choice: Choice { card, act, pay },
    });
}

/// Buttons for each way to pay for something; `label` names the action.
fn pay_buttons(
    g: &GameView,
    cost: Cost,
    card: u8,
    act: Act,
    label: String,
    style: &'static str,
) -> AnyView {
    match cost {
        Cost::Free => view! {
            <button
                class=format!("{BTN} {style}")
                on:click=move |_| choose(card, act, Pay::default())
            >
                {format!("{label} (free)")}
            </button>
        }
        .into_any(),
        Cost::Chain(from) => view! {
            <button
                class=format!("{BTN} {style}")
                on:click=move |_| choose(card, act, Pay::default())
            >
                {format!("{label}: free thanks to {from} 🔗")}
            </button>
        }
        .into_any(),
        Cost::Pay(ways) => ways
            .into_iter()
            .map(|pay| {
                let label = label.clone();
                view! {
                    <button
                        class=format!("{BTN} {style} flex flex-wrap items-center gap-1.5")
                        on:click=move |_| choose(card, act, pay)
                    >
                        <span>{label}</span>
                        {pay_chips(g, &pay)}
                    </button>
                }
            })
            .collect_view()
            .into_any(),
        Cost::Cannot(why) => view! {
            <button class=format!("{BTN} {style}") disabled=true>
                {format!("{label}: {why}")}
            </button>
        }
        .into_any(),
    }
}

/// What the player can do with card `id`: build it, raise the monument
/// with it, or sell it.
fn actions(g: &GameView, id: u8) -> AnyView {
    let cities = g.cities();
    let me = g.you as usize;
    let city = &cities[me];
    let d = def(id);
    let build = pay_buttons(
        g,
        build_cost(&cities, me, id),
        id,
        Act::Build,
        format!("🔨 Build {}", d.name),
        BTN_GO,
    );
    let power = can_use_power(city, id).then(|| {
        view! {
            <button
                class=format!("{BTN} {BTN_GO}")
                on:click=move |_| choose(id, Act::Power, Pay::default())
            >
                {format!("Build {} free with {} (once per age)", d.name, city.board().monument)}
            </button>
        }
    });
    let stage = stage_cost(&cities, me).map(|cost| {
        let next = &city.stage_defs()[city.stages as usize];
        let gifts: Vec<String> = next.gifts.iter().map(|&g| gift_face(g)).collect();
        let label = format!(
            "🏛️ Stage {} ({} → {})",
            city.stages + 1,
            icons(next.cost),
            gifts.join(" ")
        );
        let explain = stage_text(next.gifts);
        view! {
            {pay_buttons(g, cost, id, Act::Stage, label, BTN_ALT)}
            <p class="px-1 text-xs text-stone-500 dark:text-stone-400">
                {format!("Stage {} gives {explain}.", city.stages + 1)}
            </p>
        }
    });
    view! {
        <div class="space-y-2 rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100">
            {card_detail(id)}
            <p class="flex items-center gap-1 pt-1 text-xs text-stone-500 dark:text-stone-400">
                "You have " {pill(PILL_COINS, "Your coins", format!("🟡 {}", city.coins))}
            </p>
            <div class="space-y-1.5">
                {build} {power} {stage}
                <button
                    class=format!("{BTN} {BTN_SELL}")
                    on:click=move |_| choose(id, Act::Sell, Pay::default())
                >
                    "💰 Sell it for 🟡3"
                </button>
            </div>
        </div>
    }
    .into_any()
}

/// Petra's monument: build a discarded card for free.
#[component]
fn SalvagePanel(game: Memo<GameView>, inspect: RwSignal<Option<u8>>) -> impl IntoView {
    let selected = RwSignal::new(None::<u8>);
    move || {
        let g = game.get();
        let Stage::Salvage { player } = g.stage else {
            return None;
        };
        if player != g.you {
            return None;
        }
        let city = &g.players[g.you as usize].city;
        let cards = g.discard.clone();
        let picked = selected.get().filter(|id| cards.contains(id));
        Some(view! {
            <div class="space-y-3 rounded-lg border-2 border-amber-400 bg-amber-50 p-3 dark:bg-stone-800">
                <p class="text-sm font-semibold text-stone-800 dark:text-stone-100">
                    {format!(
                        "♻️ {}: build one discarded card for free.",
                        city.board().monument,
                    )}
                </p>
                <div class="grid grid-cols-4 gap-1.5 sm:grid-cols-7">
                    {cards
                        .into_iter()
                        .map(|id| {
                            let have = city.owns(def(id).name);
                            view! {
                                <button
                                    class=move || {
                                        let mut c = String::from(
                                            "touch-manipulation rounded-lg text-left",
                                        );
                                        if selected.get() == Some(id) {
                                            c.push_str(" mon-pick");
                                        }
                                        if have {
                                            c.push_str(" mon-dim");
                                        }
                                        c
                                    }
                                    on:click=move |_| {
                                        if have {
                                            inspect.set(Some(id));
                                        } else {
                                            selected.set(Some(id));
                                        }
                                    }
                                >
                                    {card_face(id)}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                {picked
                    .map(|id| {
                        view! {
                            <div class="space-y-2 rounded-lg bg-white p-3 dark:bg-stone-900 dark:text-stone-100">
                                {card_detail(id)}
                                <button
                                    class=format!("{BTN} {BTN_GO}")
                                    on:click=move |_| send_msg(
                                        &ClientMsg::Salvage {
                                            card: Some(id),
                                        },
                                    )
                                >
                                    {format!("Build {} for free", def(id).name)}
                                </button>
                            </div>
                        }
                    })}
                <button
                    class=format!("{BTN} {BTN_SELL}")
                    on:click=|_| send_msg(&ClientMsg::Salvage { card: None })
                >
                    "Build nothing"
                </button>
            </div>
        })
    }
}

/// One player's city: monument, resources, military, science and every
/// card they've built. Tap a card to read it.
fn city_panel(g: &GameView, id: u8, inspect: RwSignal<Option<u8>>) -> AnyView {
    let Some(p) = g.players.iter().find(|p| p.id == id) else {
        return view! { <p class="text-sm text-stone-400">"Nobody here."</p> }.into_any();
    };
    let cities = g.cities();
    let city: &City = &p.city;
    let board = city.board();
    let mine = id == g.you;
    let me = &cities[g.you as usize];
    let side = format!("{:?}", city.side);
    let relation = if mine {
        "Your city".to_string()
    } else if id == g.left() {
        format!("{}'s city · your left neighbor", p.nickname)
    } else if id == g.right() {
        format!("{}'s city · your right neighbor", p.nickname)
    } else {
        format!("{}'s city", p.nickname)
    };
    let points = score(&cities, id as usize);
    let (sci, wild) = city.science();
    let tokens = city
        .tokens
        .iter()
        .map(|&t| {
            let (text, class, title) = if t > 0 {
                (format!("+{t}"), "bg-green-600 text-white ring-green-800", "War victory")
            } else {
                (format!("{t}"), "bg-red-600 text-white ring-red-800", "War defeat")
            };
            view! {
                <span
                    title=title
                    class=format!(
                        "inline-flex h-6 min-w-6 items-center justify-center rounded-full px-1 text-xs font-bold ring-1 {class}",
                    )
                >
                    {format!("⚔️{text}")}
                </span>
            }
        })
        .collect_view();

    // What they make. For a neighbor, what it costs you to buy.
    let buyer = if mine {
        None
    } else if id == g.left() {
        Some((me, true))
    } else if id == g.right() {
        Some((me, false))
    } else {
        None
    };
    let makes = resource_chips(city, buyer);

    let stages = city
        .stage_defs()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let built = (i as u8) < city.stages;
            let gifts: Vec<String> = s.gifts.iter().map(|&g| gift_face(g)).collect();
            let title: Vec<String> = s.gifts.iter().map(|&g| gift_text(g)).collect();
            view! {
                <div
                    title=title.join("; ")
                    class=if built {
                        "min-w-0 flex-1 rounded-md bg-amber-400 px-1.5 py-1 text-stone-900 ring-1 ring-amber-600"
                    } else {
                        "min-w-0 flex-1 rounded-md bg-white/70 px-1.5 py-1 text-stone-700 ring-1 ring-stone-300 dark:bg-stone-800 dark:text-stone-200 dark:ring-stone-600"
                    }
                >
                    <div class="flex items-center justify-between text-[0.6rem] font-semibold uppercase opacity-70">
                        <span>{format!("Stage {}", i + 1)}</span>
                        {built.then(|| view! { <span>"✓"</span> })}
                    </div>
                    <div class="truncate text-xs">{icons(s.cost)}</div>
                    <div class="text-sm font-bold leading-tight">{gifts.join(" ")}</div>
                </div>
            }
        })
        .collect_view();
    // What each stage does, spelled out: icons alone don't say enough.
    let stage_help = city
        .stage_defs()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let built = (i as u8) < city.stages;
            view! {
                <li class="flex gap-1.5">
                    <span class=if built {
                        "w-4 shrink-0 font-semibold text-amber-600 dark:text-amber-400"
                    } else {
                        "w-4 shrink-0 font-semibold"
                    }>{if built { "✓".to_string() } else { format!("{}.", i + 1) }}</span>
                    <span>{capitalized(&stage_text(s.gifts))}</span>
                </li>
            }
        })
        .collect_view();

    let groups = Color::ALL
        .into_iter()
        .filter_map(|color| {
            let ids: Vec<u8> = city
                .built
                .iter()
                .copied()
                .filter(|&c| def(c).color == color)
                .collect();
            if ids.is_empty() {
                return None;
            }
            Some(view! {
                <div class="flex flex-wrap items-center gap-1">
                    {ids
                        .into_iter()
                        .map(|c| {
                            let d = def(c);
                            let (effect, _) = effect_face(d.effect);
                            view! {
                                <button
                                    data-card=mine.then_some(c)
                                    class=format!(
                                        "flex max-w-full touch-manipulation items-center gap-1 rounded-md px-1.5 py-1 text-left text-xs shadow-sm {}",
                                        band(d.color),
                                    )
                                    on:click=move |_| inspect.set(Some(c))
                                >
                                    <span class="truncate font-semibold">{d.name}</span>
                                    <span class="shrink-0 rounded bg-white/90 px-1 text-stone-900">
                                        {effect}
                                    </span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            })
        })
        .collect_view();

    let last = p.last.map(|r| done_text(g, r.done, &r.pay));

    view! {
        <div class="space-y-3 text-stone-800 dark:text-stone-100">
            <div>
                <p class="text-xs text-stone-500 dark:text-stone-400">{relation}</p>
                <p class="font-bold">
                    {format!("{} · {}", board.city, board.monument)}
                    <span class="ml-1 rounded bg-stone-300 px-1 text-[0.6rem] font-semibold text-stone-700 dark:bg-stone-600 dark:text-stone-100">
                        {format!("side {side}")}
                    </span>
                </p>
            </div>
            <div class="flex flex-wrap items-center gap-1.5">
                {pill(PILL_COINS, "Coins", format!("🟡 {}", city.coins))}
                {pill(PILL_SHIELDS, "Shields", format!("🛡️ {}", city.shields()))}
                {pill(PILL_POINTS, "Points if the game ended now", format!("⭐ {}", points.total))}
                {pill(
                    PILL_SCIENCE,
                    "Science symbols",
                    view! {
                        {Sci::ALL
                            .into_iter()
                            .zip(sci)
                            .map(|(s, n)| {
                                view! {
                                    <span class=if n == 0 {
                                        "opacity-40"
                                    } else {
                                        ""
                                    }>{format!("{}{n}", s.icon())}</span>
                                }
                            })
                            .collect_view()}
                        {(wild > 0).then(|| view! { <span>{format!("+{wild}❓")}</span> })}
                    },
                )} {tokens}
            </div>
            <div class="space-y-1.5">
                <div class="flex gap-1">{stages}</div>
                <ul class="space-y-0.5 text-xs text-stone-600 dark:text-stone-300">{stage_help}</ul>
            </div>
            <div class="flex flex-wrap items-center gap-1">
                <span class="text-xs text-stone-500 dark:text-stone-400">"Makes"</span>
                {makes}
            </div>
            <div class="space-y-1">
                {if city.built.is_empty() {
                    view! { <p class="text-xs text-stone-500">"Nothing built yet."</p> }.into_any()
                } else {
                    groups.into_any()
                }}
            </div>
            {last
                .map(|l| {
                    view! {
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            {format!("Last turn: {l}")}
                        </p>
                    }
                })}
        </div>
    }
    .into_any()
}

fn score_table(g: &GameView, scores: Vec<Breakdown>) -> impl IntoView {
    let rows = scores
        .into_iter()
        .map(|s| {
            view! {
                <tr class="border-t border-stone-200 dark:border-stone-700">
                    <td class="py-1.5 pr-2 font-semibold">{g.name(s.player)}</td>
                    <td class="px-1 text-right">{s.military}</td>
                    <td class="px-1 text-right">{s.treasury}</td>
                    <td class="px-1 text-right">{s.monument}</td>
                    <td class="px-1 text-right">{s.civic}</td>
                    <td class="px-1 text-right">{s.science}</td>
                    <td class="px-1 text-right">{s.commerce}</td>
                    <td class="px-1 text-right">{s.leagues}</td>
                    <td class="pl-2 text-right font-bold">{s.total}</td>
                </tr>
            }
        })
        .collect_view();
    view! {
        <div class="overflow-x-auto rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
            <table class="game-table w-full text-sm dark:text-stone-100">
                <thead>
                    <tr class="text-xs text-stone-500">
                        <th class="pb-1 text-left">"Player"</th>
                        <th class="px-1 text-right" title="War">
                            "⚔️"
                        </th>
                        <th class="px-1 text-right" title="Coins">
                            "🟡"
                        </th>
                        <th class="px-1 text-right" title="Monument">
                            "🏛️"
                        </th>
                        <th class="px-1 text-right" title="Civic">
                            {swatch(Color::Blue)}
                        </th>
                        <th class="px-1 text-right" title="Science">
                            {swatch(Color::Green)}
                        </th>
                        <th class="px-1 text-right" title="Commerce">
                            {swatch(Color::Yellow)}
                        </th>
                        <th class="px-1 text-right" title="Leagues">
                            {swatch(Color::Purple)}
                        </th>
                        <th class="pl-2 text-right">"Total"</th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[component]
pub fn MonumentsLobby() -> impl IntoView {
    cards::lobby_page(GameKind::Monuments, options_form().into_any(), rules_button)
}

#[component]
pub fn MonumentsRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::Monuments,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
