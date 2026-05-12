// Jackson Coxson
#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

use leptos::prelude::*;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};
use wasm_bindgen::JsValue;

use super::state::LogLevel;

static LEVEL: AtomicU8 = AtomicU8::new(3);
static INSTALLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static LOG_SINK: RefCell<Option<RwSignal<Vec<String>>>> = const { RefCell::new(None) };
}

pub fn install() {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = tracing::subscriber::set_global_default(WebSubscriber::default());
}

pub fn attach_sink(sig: RwSignal<Vec<String>>) {
    LOG_SINK.with(|s| *s.borrow_mut() = Some(sig));
}

pub fn set_level(level: LogLevel) {
    LEVEL.store(encode(level), Ordering::Relaxed);
}

fn encode(level: LogLevel) -> u8 {
    match level {
        LogLevel::Off => 0,
        LogLevel::Error => 1,
        LogLevel::Warn => 2,
        LogLevel::Info => 3,
        LogLevel::Debug => 4,
        LogLevel::Trace => 5,
    }
}

fn current_threshold() -> Option<Level> {
    match LEVEL.load(Ordering::Relaxed) {
        0 => None,
        1 => Some(Level::ERROR),
        2 => Some(Level::WARN),
        3 => Some(Level::INFO),
        4 => Some(Level::DEBUG),
        5 => Some(Level::TRACE),
        _ => Some(Level::INFO),
    }
}

#[derive(Default)]
struct WebSubscriber {
    next_id: AtomicU64,
}

impl Subscriber for WebSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        match current_threshold() {
            None => false,
            Some(threshold) => *metadata.level() <= threshold,
        }
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        Id::from_u64(n)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let line = format_line(meta, &visitor);
        let js = JsValue::from_str(&line);
        match *meta.level() {
            Level::ERROR => web_sys::console::error_1(&js),
            Level::WARN => web_sys::console::warn_1(&js),
            Level::INFO => web_sys::console::log_1(&js),
            Level::DEBUG => web_sys::console::debug_1(&js),
            Level::TRACE => web_sys::console::trace_1(&js),
        }

        LOG_SINK.with(|s| {
            if let Some(sig) = s.borrow().as_ref() {
                let _ = sig.try_update(|v| v.push(line));
            }
        });
    }
}

fn format_line(meta: &Metadata<'_>, visitor: &FieldVisitor) -> String {
    let target = meta.target();
    let level = meta.level();
    let msg = visitor.message.as_deref().unwrap_or("");
    if visitor.extras.is_empty() {
        format!("[{level:5}] {target}: {msg}")
    } else {
        format!("[{level:5}] {target}: {msg} {}", visitor.extras.join(" "))
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    extras: Vec<String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.extras.push(format!("{}={value:?}", field.name()));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.extras.push(format!("{}={value:?}", field.name()));
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.extras.push(format!("{}={value}", field.name()));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.extras.push(format!("{}={value}", field.name()));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.extras.push(format!("{}={value}", field.name()));
    }
}
