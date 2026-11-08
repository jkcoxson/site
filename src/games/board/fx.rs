// Jackson Coxson
// Pieces flying onto the board (browser only). A copy of the piece is drawn
// over the page, sized as it will be on the board, and animated from its
// starting pose to its spot; then the game shows the real piece in place.

use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use super::{ScreenPose, FLY_MS};
use crate::games::cards::fx::{after, animate, reduced_motion};

pub fn fly(
    html: String,
    from: ScreenPose,
    to: ScreenPose,
    delay: i32,
    landed: impl FnOnce() + 'static,
) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        landed();
        return;
    };
    if reduced_motion() || to.len <= 0.0 {
        after(delay, landed);
        return;
    }
    let (Some(body), Ok(el)) = (
        doc.body(),
        doc.create_element("div")
            .map_err(|_| ())
            .and_then(|e| e.dyn_into::<HtmlElement>().map_err(|_| ())),
    ) else {
        landed();
        return;
    };
    el.set_inner_html(&html);
    let _ = el.class_list().add_1("board-ghost");
    let (w, h) = (to.len, to.len / 2.0);
    let style = el.style();
    for (k, v) in [
        ("left", format!("{}px", to.x - w / 2.0)),
        ("top", format!("{}px", to.y - h / 2.0)),
        ("width", format!("{w}px")),
        ("height", format!("{h}px")),
        // Hidden until its delay is up.
        ("opacity", "0".to_string()),
    ] {
        let _ = style.set_property(&k, &v);
    }
    if body.append_child(&el).is_err() {
        landed();
        return;
    }
    let scale = from.len / to.len;
    // Turn the short way round.
    let mut start = from.angle;
    while start - to.angle > 180.0 {
        start -= 360.0;
    }
    while to.angle - start > 180.0 {
        start += 360.0;
    }
    let (dx, dy) = (from.x - to.x, from.y - to.y);
    let lift = scale.max(1.0) * 1.18;
    animate(
        &el,
        &[
            &[
                (
                    "transform",
                    format!("translate({dx}px, {dy}px) rotate({start}deg) scale({scale})"),
                ),
                ("opacity", "1".into()),
                ("offset", "0".into()),
            ],
            &[
                (
                    "transform",
                    format!(
                        "translate({}px, {}px) rotate({}deg) scale({lift})",
                        dx * 0.35,
                        dy * 0.35,
                        start + (to.angle - start) * 0.7
                    ),
                ),
                ("opacity", "1".into()),
                ("offset", "0.6".into()),
            ],
            &[
                ("transform", format!("rotate({}deg) scale(1)", to.angle)),
                ("opacity", "1".into()),
                ("offset", "1".into()),
            ],
        ],
        FLY_MS as f64,
        delay as f64,
        "cubic-bezier(.3,.7,.3,1)",
        "both",
    );
    after(delay + FLY_MS, move || {
        landed();
        // One more frame so the real piece is drawn before the copy goes.
        after(30, move || el.remove());
    });
}
