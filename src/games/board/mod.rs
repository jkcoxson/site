// Jackson Coxson
// A board that's bigger than the screen, for any game laid out on a table
// (dominoes, tiles, maps). Games draw pieces into a "world" measured in their
// own units; the camera pans and zooms that world inside a viewport.
//
// - Drag to pan, pinch or scroll to zoom, and buttons to zoom and fit.
// - The game can move the camera too (`fit`, `reveal`), and those moves glide
//   so players can follow where the action went.
// - `fly` sends a piece across the page from anywhere (a hand, a seat) to its
//   spot on the board, landing where the camera will be once it's done
//   moving.

#[cfg(target_arch = "wasm32")]
mod fx;

use leptos::prelude::*;

/// Pixels per world unit inside the world layer, before the camera's zoom.
/// Pieces are drawn at this size and scaled, so keep it near the size
/// they're usually seen at to keep them sharp.
pub const UNIT: f64 = 40.0;

/// How long a camera glide takes, in milliseconds.
pub const GLIDE_MS: i32 = 450;

/// The most and least zoomed in, in screen pixels per world unit.
const MAX_ZOOM: f64 = 90.0;
const MIN_ZOOM: f64 = 4.0;
/// A piece is too small to read below this zoom, so revealing one zooms in.
const READABLE_ZOOM: f64 = 16.0;

/// An axis-aligned box in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn around(x: f64, y: f64, r: f64) -> Self {
        Bounds {
            min_x: x - r,
            min_y: y - r,
            max_x: x + r,
            max_y: y + r,
        }
    }

    pub fn union(self, o: Bounds) -> Self {
        Bounds {
            min_x: self.min_x.min(o.min_x),
            min_y: self.min_y.min(o.min_y),
            max_x: self.max_x.max(o.max_x),
            max_y: self.max_y.max(o.max_y),
        }
    }

    pub fn grow(self, by: f64) -> Self {
        Bounds {
            min_x: self.min_x - by,
            min_y: self.min_y - by,
            max_x: self.max_x + by,
            max_y: self.max_y + by,
        }
    }

    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    fn middle(&self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    fn contains(&self, o: &Bounds) -> bool {
        o.min_x >= self.min_x
            && o.max_x <= self.max_x
            && o.min_y >= self.min_y
            && o.max_y <= self.max_y
    }
}

/// What the camera is looking at: the world point in the middle of the
/// viewport, and the zoom in screen pixels per world unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

/// A piece on screen: its center in page (client) pixels, its length along
/// its long side in pixels, and its rotation in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPose {
    pub x: f64,
    pub y: f64,
    pub len: f64,
    pub angle: f64,
}

/// The camera over one board. Cheap to copy into closures.
#[derive(Clone, Copy)]
pub struct Camera {
    /// Where the camera is headed. Everything that measures the board
    /// (fitting, revealing, flying pieces in) works from here.
    view: RwSignal<View>,
    /// Where it's drawn right now, partway through a glide.
    drawn: RwSignal<View>,
    /// Counts camera moves, so a glide stops when another move starts.
    moves: StoredValue<u32>,
    /// Viewport size in pixels.
    size: RwSignal<(f64, f64)>,
    /// Everything on the board, for "fit" and how far out you can zoom.
    world: RwSignal<Bounds>,
    viewport: NodeRef<leptos::html::Div>,
    /// The pointer moved the board since it went down, so the tap that
    /// ends it isn't a tap on a piece.
    dragged: StoredValue<bool>,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self {
            view: RwSignal::new(START),
            drawn: RwSignal::new(START),
            moves: StoredValue::new(0),
            size: RwSignal::new((360.0, 400.0)),
            world: RwSignal::new(Bounds::around(0.0, 0.0, 2.0)),
            viewport: NodeRef::new(),
            dragged: StoredValue::new(false),
        }
    }

    /// Tell the camera how big the board is now.
    pub fn set_world(&self, b: Bounds) {
        self.world.set(b);
    }

    pub fn view(&self) -> View {
        self.view.get_untracked()
    }

    /// Whether the latest pointer gesture was a drag rather than a tap.
    /// Pieces that react to taps should check this first.
    pub fn was_drag(&self) -> bool {
        self.dragged.get_value()
    }

    fn min_zoom(&self) -> f64 {
        let fit = self.zoom_to_fit(self.world.get_untracked().grow(1.0));
        (fit * 0.7).clamp(MIN_ZOOM, MAX_ZOOM)
    }

    fn zoom_to_fit(&self, b: Bounds) -> f64 {
        let (w, h) = self.size.get_untracked();
        (w / b.width().max(0.1)).min(h / b.height().max(0.1))
    }

    fn go(&self, view: View, glide: bool) {
        let zoom = view.zoom.clamp(self.min_zoom().min(MAX_ZOOM), MAX_ZOOM);
        let to = View { zoom, ..view };
        self.view.set(to);
        self.moves.update_value(|n| *n = n.wrapping_add(1));
        #[cfg(target_arch = "wasm32")]
        if glide && !crate::games::cards::fx::reduced_motion() {
            let from = self.drawn.get_untracked();
            glide_frame(*self, self.moves.get_value(), js_sys::Date::now(), from, to);
            return;
        }
        let _ = glide;
        self.drawn.set(to);
    }

    /// Stop any glide where it is, so a finger can take over from there.
    fn hold(&self) {
        let here = self.drawn.get_untracked();
        self.moves.update_value(|n| *n = n.wrapping_add(1));
        self.view.set(here);
    }

    /// Frame `b`, zoomed in no further than `max_zoom`.
    pub fn fit(&self, b: Bounds, max_zoom: f64, glide: bool) {
        self.measure();
        let b = b.grow(0.8);
        let (x, y) = b.middle();
        let zoom = self.zoom_to_fit(b).min(max_zoom);
        self.go(View { x, y, zoom }, glide);
    }

    /// Frame the whole board.
    pub fn fit_all(&self, glide: bool) {
        self.fit(self.world.get_untracked(), MAX_ZOOM * 0.5, glide);
    }

    /// The part of the world on screen, in world units.
    fn visible(&self, view: View) -> Bounds {
        let (w, h) = self.size.get_untracked();
        let (hw, hh) = (w / 2.0 / view.zoom, h / 2.0 / view.zoom);
        Bounds {
            min_x: view.x - hw,
            min_y: view.y - hh,
            max_x: view.x + hw,
            max_y: view.y + hh,
        }
    }

    /// Bring `b` into view if it isn't already, moving as little as
    /// possible. Returns whether the camera moved.
    pub fn reveal(&self, b: Bounds) -> bool {
        self.measure();
        let view = self.view.get_untracked();
        let margin = 0.5;
        if view.zoom >= READABLE_ZOOM && self.visible(view).grow(-margin).contains(&b) {
            return false;
        }
        let zoom = view.zoom.max(READABLE_ZOOM * 1.4);
        let seen = self.visible(View { zoom, ..view }).grow(-margin);
        if seen.width() < b.width() || seen.height() < b.height() {
            self.fit(b, MAX_ZOOM * 0.6, true);
            return true;
        }
        // Slide just far enough.
        let dx = if b.min_x < seen.min_x {
            b.min_x - seen.min_x
        } else if b.max_x > seen.max_x {
            b.max_x - seen.max_x
        } else {
            0.0
        };
        let dy = if b.min_y < seen.min_y {
            b.min_y - seen.min_y
        } else if b.max_y > seen.max_y {
            b.max_y - seen.max_y
        } else {
            0.0
        };
        self.go(
            View {
                x: view.x + dx,
                y: view.y + dy,
                zoom,
            },
            true,
        );
        true
    }

    fn zoom_by(&self, factor: f64, around: Option<(f64, f64)>, glide: bool) {
        let view = self.view.get_untracked();
        let zoom = (view.zoom * factor).clamp(self.min_zoom(), MAX_ZOOM);
        // Keep the world point under `around` (viewport pixels from the
        // middle) where it is.
        let (ax, ay) = around.unwrap_or((0.0, 0.0));
        let (wx, wy) = (view.x + ax / view.zoom, view.y + ay / view.zoom);
        self.go(
            View {
                x: wx - ax / zoom,
                y: wy - ay / zoom,
                zoom,
            },
            glide,
        );
    }

    /// Where a world point will be on the page once the camera stops.
    pub fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        let view = self.view.get_untracked();
        let (w, h) = self.size.get_untracked();
        let (left, top) = self.origin();
        (
            left + w / 2.0 + (x - view.x) * view.zoom,
            top + h / 2.0 + (y - view.y) * view.zoom,
        )
    }

    /// A piece `len` world units long at a world pose, on the page.
    pub fn pose_on_screen(&self, x: f64, y: f64, len: f64, angle: f64) -> ScreenPose {
        let (sx, sy) = self.to_screen(x, y);
        ScreenPose {
            x: sx,
            y: sy,
            len: len * self.view.get_untracked().zoom,
            angle,
        }
    }

    /// The viewport's top-left corner on the page.
    fn origin(&self) -> (f64, f64) {
        #[cfg(target_arch = "wasm32")]
        if let Some(el) = self.viewport.get_untracked() {
            let r = el.get_bounding_client_rect();
            return (r.left(), r.top());
        }
        (0.0, 0.0)
    }

    /// Read the viewport's size from the page.
    pub fn measure(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(el) = self.viewport.get_untracked() {
            let r = el.get_bounding_client_rect();
            if r.width() > 0.0 && r.height() > 0.0 {
                let size = (r.width(), r.height());
                if self.size.get_untracked() != size {
                    self.size.set(size);
                }
            }
        }
    }

    fn transform(&self) -> String {
        let view = self.drawn.get();
        let (w, h) = self.size.get();
        format!(
            "transform: translate({:.2}px, {:.2}px) scale({:.4})",
            w / 2.0 - view.x * view.zoom,
            h / 2.0 - view.y * view.zoom,
            view.zoom / UNIT
        )
    }
}

const START: View = View {
    x: 0.0,
    y: 0.0,
    zoom: 30.0,
};

/// One frame of a camera glide. The glide is drawn frame by frame rather
/// than with a CSS transition: phones draw a transitioning layer whole, at
/// its largest zoom, and a big board runs them out of memory, so the board
/// vanishes until the transition ends.
#[cfg(target_arch = "wasm32")]
fn glide_frame(camera: Camera, id: u32, start: f64, from: View, to: View) {
    crate::games::cards::fx::next_frame(move || {
        if camera.moves.get_value() != id {
            return;
        }
        let t = ((js_sys::Date::now() - start) / GLIDE_MS as f64).clamp(0.0, 1.0);
        // Ease out, like the rest of the site's motion.
        let e = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: f64, b: f64| a + (b - a) * e;
        camera.drawn.set(View {
            x: lerp(from.x, to.x),
            y: lerp(from.y, to.y),
            // Zoom evenly in scale, not in pixels.
            zoom: from.zoom * (to.zoom / from.zoom).powf(e),
        });
        if t < 1.0 {
            glide_frame(camera, id, start, from, to);
        }
    });
}

/// Inline style that puts a piece `w`×`h` world units at (`x`, `y`) (its
/// center), turned `angle` degrees, inside the board's world.
pub fn piece_style(x: f64, y: f64, w: f64, h: f64, angle: f64) -> String {
    format!(
        "left: {:.2}px; top: {:.2}px; width: {:.2}px; height: {:.2}px; transform: rotate({:.2}deg)",
        (x - w / 2.0) * UNIT,
        (y - h / 2.0) * UNIT,
        w * UNIT,
        h * UNIT,
        angle
    )
}

/// Fly `html` (a piece's markup, drawn lying flat, long side across) from
/// `from` to `to` on the page after `delay` ms, then run `landed`. With
/// reduced motion it lands at once.
pub fn fly(
    html: String,
    from: ScreenPose,
    to: ScreenPose,
    delay: i32,
    landed: impl FnOnce() + 'static,
) {
    #[cfg(target_arch = "wasm32")]
    fx::fly(html, from, to, delay, landed);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (html, from, to, delay);
        landed();
    }
}

/// How long `fly` takes once it starts.
pub const FLY_MS: i32 = 560;

/// A pointer on the board, for panning and pinching.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy)]
struct Touch {
    id: i32,
    x: f64,
    y: f64,
}

/// The board: `world` is drawn in world units (see `piece_style`) and
/// moved by `camera`. Zoom and fit buttons sit in the top corner.
pub fn board_view(camera: Camera, world: impl IntoView + 'static) -> impl IntoView {
    let touches = StoredValue::new(Vec::<Touch>::new());
    let start = StoredValue::new((0.0f64, 0.0f64));

    #[cfg(target_arch = "wasm32")]
    {
        Effect::new(move |_| {
            camera.measure();
        });
        let handle = window_event_listener(leptos::ev::resize, move |_| camera.measure());
        on_cleanup(move || handle.remove());
    }

    let local = move |x: f64, y: f64| {
        let (left, top) = camera.origin();
        let (w, h) = camera.size.get_untracked();
        (x - left - w / 2.0, y - top - h / 2.0)
    };

    let on_down = move |ev: web_sys::PointerEvent| {
        let t = Touch {
            id: ev.pointer_id(),
            x: ev.client_x() as f64,
            y: ev.client_y() as f64,
        };
        touches.update_value(|ts| {
            ts.retain(|o| o.id != t.id);
            ts.push(t);
        });
        camera.hold();
        if touches.with_value(|ts| ts.len()) == 1 {
            camera.dragged.set_value(false);
            start.set_value((t.x, t.y));
        }
    };

    let on_move = move |ev: web_sys::PointerEvent| {
        let id = ev.pointer_id();
        let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
        let before = touches.with_value(|ts| ts.clone());
        let Some(old) = before.iter().find(|t| t.id == id).copied() else {
            return;
        };
        if !camera.dragged.get_value() {
            let (sx, sy) = start.get_value();
            if (x - sx).hypot(y - sy) < 6.0 && before.len() < 2 {
                return;
            }
            camera.dragged.set_value(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(el) = camera.viewport.get_untracked() {
                for t in &before {
                    let _ = el.set_pointer_capture(t.id);
                }
            }
        }
        let after: Vec<Touch> = before
            .iter()
            .map(|t| if t.id == id { Touch { id, x, y } } else { *t })
            .collect();
        touches.set_value(after.clone());
        let view = camera.view.get_untracked();
        if before.len() >= 2 {
            let (a0, b0) = (before[0], before[1]);
            let (a1, b1) = (after[0], after[1]);
            let d0 = (a0.x - b0.x).hypot(a0.y - b0.y).max(1.0);
            let d1 = (a1.x - b1.x).hypot(a1.y - b1.y).max(1.0);
            let m0 = local((a0.x + b0.x) / 2.0, (a0.y + b0.y) / 2.0);
            let m1 = local((a1.x + b1.x) / 2.0, (a1.y + b1.y) / 2.0);
            let zoom = (view.zoom * d1 / d0).clamp(camera.min_zoom(), MAX_ZOOM);
            let (wx, wy) = (view.x + m0.0 / view.zoom, view.y + m0.1 / view.zoom);
            camera.go(
                View {
                    x: wx - m1.0 / zoom,
                    y: wy - m1.1 / zoom,
                    zoom,
                },
                false,
            );
        } else {
            camera.go(
                View {
                    x: view.x - (x - old.x) / view.zoom,
                    y: view.y - (y - old.y) / view.zoom,
                    zoom: view.zoom,
                },
                false,
            );
        }
    };

    let on_up = move |ev: web_sys::PointerEvent| {
        let id = ev.pointer_id();
        touches.update_value(|ts| ts.retain(|t| t.id != id));
        if touches.with_value(|ts| ts.len()) == 1 {
            // Pinch to one finger: carry on panning from here.
            let t = touches.with_value(|ts| ts[0]);
            start.set_value((t.x, t.y));
        }
    };

    let on_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        camera.hold();
        let factor = (-ev.delta_y() * 0.0015).exp();
        let at = local(ev.client_x() as f64, ev.client_y() as f64);
        camera.zoom_by(factor, Some(at), false);
    };

    let button = "flex h-9 w-9 touch-manipulation items-center justify-center rounded-full bg-stone-900/70 text-lg font-bold text-white shadow ring-1 ring-white/20 backdrop-blur hover:bg-stone-900/90";
    view! {
        <div
            node_ref=camera.viewport
            class="board-viewport"
            on:pointerdown=on_down
            on:pointermove=on_move
            on:pointerup=on_up
            on:pointercancel=on_up
            on:wheel=on_wheel
            on:contextmenu=|ev| ev.prevent_default()
        >
            <div
                class="board-world"
                style=move || camera.transform()
            >
                {world}
            </div>
            <div
                class="absolute top-2 right-2 flex flex-col gap-2"
                on:pointerdown=|ev| ev.stop_propagation()
            >
                <button class=button aria-label="Zoom in" on:click=move |_| camera.zoom_by(1.5, None, true)>
                    "+"
                </button>
                <button class=button aria-label="Zoom out" on:click=move |_| camera.zoom_by(1.0 / 1.5, None, true)>
                    "−"
                </button>
                <button class=button aria-label="Show the whole board" title="Show the whole board" on:click=move |_| camera.fit_all(true)>
                    <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
                    </svg>
                </button>
            </div>
        </div>
    }
}
