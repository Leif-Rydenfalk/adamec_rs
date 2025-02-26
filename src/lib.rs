use core::marker::PhantomData;
use dominator::clone;
use dominator::{class, events, html, Dom};
use futures_signals::signal::{Mutable, SignalExt};
use futures_signals::signal_vec::MutableVec;
use futures_signals::signal_vec::SignalVecExt;
use once_cell::sync::Lazy;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use web_sys::{console, DomParser, Node};

/// A wrapper for a web_sys::Node that can be stored in a static.
struct NodeSync(Node);

// Mark NodeSync as both Sync and Send (WASM is single-threaded).
unsafe impl Sync for NodeSync {}
unsafe impl Send for NodeSync {}

/// A macro that parses a static HTML string and produces a `dominator::Dom`.
#[macro_export]
macro_rules! raw_html {
    ($html:literal) => {{
        fn get_parsed_dom() -> dominator::Dom {
            use once_cell::sync::Lazy;
            use web_sys::{DomParser, SupportedType};

            static PARSED_NODE: Lazy<NodeSync> = Lazy::new(|| {
                let parser = DomParser::new().expect("Failed to create DomParser");
                let doc = parser
                    .parse_from_string($html, SupportedType::TextHtml)
                    .expect("Failed to parse HTML string");
                let body = doc.body().expect("Parsed document has no body");
                let fragment = doc.create_document_fragment();
                let children = body.child_nodes();
                for i in 0..children.length() {
                    let child = children.item(i).expect("Child exists");
                    let clone = child
                        .clone_node_with_deep(true)
                        .expect("Failed to clone node");
                    fragment
                        .append_child(&clone)
                        .expect("Failed to append child");
                }
                NodeSync(fragment.into())
            });

            dominator::Dom::new(
                PARSED_NODE
                    .0
                    .clone_node_with_deep(true)
                    .expect("Failed to clone parsed node"),
            )
        }
        get_parsed_dom()
    }};
}

/// A simple event dispatcher that wraps a listener.
#[derive(Debug)]
pub struct EventDispatcher<A, F> {
    listener: Arc<Mutex<F>>,
    argument: PhantomData<A>,
}

impl<A, F> Clone for EventDispatcher<A, F> {
    fn clone(&self) -> Self {
        Self {
            listener: self.listener.clone(),
            argument: self.argument,
        }
    }
}

impl<A, F> EventDispatcher<A, F>
where
    F: FnMut(A),
{
    pub fn new(listener: F) -> Self {
        Self {
            listener: Arc::new(Mutex::new(listener)),
            argument: PhantomData,
        }
    }

    pub fn send(&self, event: A) {
        let mut listener = self.listener.lock().unwrap();
        listener(event);
    }
}

#[derive(Clone, Copy)]
enum ButtonEvent {
    Clicked,
}

struct Button {}

impl Button {
    /// Renders a button with given children and an event handler.
    fn render<B, C, F>(children: C, on_event: F) -> Dom
    where
        B: std::borrow::BorrowMut<Dom>,
        C: IntoIterator<Item = B>,
        F: FnMut(ButtonEvent) + 'static,
    {
        static CLASS: Lazy<String> = Lazy::new(|| {
            class! {
                .style("display", "flex")
                .style("align-items", "center")
                .style("justify-content", "center")
                .style("background", "white")
                .style("border", "1px solid rgba(0, 0, 0, 0.2)")
                .style("color", "black")
                .style("padding", "0.5rem")
                .style("border-radius", "1000rem")
                .style("cursor", "pointer")
            }
            .into()
        });

        let event_dispatcher = Rc::new(EventDispatcher::new(on_event));

        html!("div", {
            .children(&mut [
                html!("div", {
                    .class(&*CLASS)
                    .children(children)
                    .event(move |_: events::Click| {
                        event_dispatcher.send(ButtonEvent::Clicked);
                    })
                })
            ])
        })
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Trash,
    Plus,
}

/// Renders the SVG markup for an icon.
fn render_icon_svg(icon: Icon) -> Dom {
    match icon {
        Icon::Trash => raw_html!(
            r#"
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16l-1.58 14.22A2 2 0 0 1 16.432 22H7.568a2 2 0 0 1-1.988-1.78zm3.345-2.853A2 2 0 0 1 9.154 2h5.692a2 2 0 0 1 1.81 1.147L18 6H6zM2 6h20m-12 5v5m4-5v5"/></svg>
            "#
        ),
        Icon::Plus => raw_html!(
            r#"
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"> <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" d="M8 3v10M3 8h10" style="stroke-width: var(--icon-weight, 2);"/></svg>
            "#
        ),
    }
}

/// Style structure for icons.
#[derive(Clone, Copy)]
pub struct IconStyle {
    pub size: f32,
    pub weight: Option<f32>,
}

impl IconStyle {
    pub fn new(size: f32) -> Self {
        Self { size, weight: None }
    }
}

/// Style structure for fonts.
#[derive(Clone, Copy)]
pub struct FontStyle {
    pub size: f32,
    pub leading: f32,
    pub weight: Option<&'static str>,
    pub style: Option<&'static str>,
}

impl FontStyle {
    pub fn new(size: f32, leading: f32) -> Self {
        Self {
            size,
            leading,
            weight: None,
            style: None,
        }
    }

    pub fn weight(mut self, weight: &'static str) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn italic(mut self) -> Self {
        self.style = Some("italic");
        self
    }
}

/// Helper that converts a font weight to an icon stroke width.
fn font_weight_to_icon_weight(weight: Option<&'static str>) -> Option<f32> {
    match weight {
        Some("bold") => Some(3.0),
        Some("600") => Some(2.5),
        Some("normal") => Some(2.0),
        _ => None,
    }
}

/// Shared CSS class for text and icons.
static STANDARD_FONT_CLASS: Lazy<String> = Lazy::new(|| {
    class! {
        .style("font-family", "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Ubuntu, sans-serif")
        .style("color", "inherit")
    }
    .into()
});

/// Creates a text helper.
fn text(text: &str) -> TextHelper {
    TextHelper { text }
}

/// Helper for rendering text with various styles.
#[derive(Clone, Copy)]
pub struct TextHelper<'a> {
    text: &'a str,
}

impl<'a> TextHelper<'a> {
    fn render_with_style(self, font_style: FontStyle) -> Dom {
        html!("div", {
            .class(&*STANDARD_FONT_CLASS)
            .style("font-size", scaled_size(font_style.size))
            .style("line-height", scaled_size(font_style.leading))
            .text(self.text)
            .apply_if(font_style.weight.is_some(), |element| {
                element.style("font-weight", font_style.weight.unwrap())
            })
            .apply_if(font_style.style.is_some(), |element| {
                element.style("font-style", font_style.style.unwrap())
            })
        })
    }

    pub fn custom(self, font_style: FontStyle) -> Dom {
        self.render_with_style(font_style)
    }

    pub fn large_title(self) -> Dom {
        self.render_with_style(FontStyle::new(34.0, 41.0).weight("bold"))
    }

    pub fn title(self) -> Dom {
        self.render_with_style(FontStyle::new(28.0, 34.0).weight("bold"))
    }

    pub fn title2(self) -> Dom {
        self.render_with_style(FontStyle::new(22.0, 28.0).weight("bold"))
    }

    pub fn title3(self) -> Dom {
        self.render_with_style(FontStyle::new(20.0, 25.0).weight("bold"))
    }

    pub fn headline(self) -> Dom {
        self.render_with_style(FontStyle::new(17.0, 22.0).weight("600"))
    }

    pub fn body(self) -> Dom {
        self.render_with_style(FontStyle::new(17.0, 22.0))
    }

    pub fn callout(self) -> Dom {
        self.render_with_style(FontStyle::new(16.0, 21.0).italic())
    }

    pub fn subheadline(self) -> Dom {
        self.render_with_style(FontStyle::new(15.0, 20.0))
    }

    pub fn footnote(self) -> Dom {
        self.render_with_style(FontStyle::new(13.0, 18.0))
    }

    pub fn caption(self) -> Dom {
        self.render_with_style(FontStyle::new(12.0, 16.0))
    }

    pub fn caption2(self) -> Dom {
        self.render_with_style(FontStyle::new(11.0, 13.0))
    }
}

/// Helper to scale text sizes.
const TEXT_SCALE: f32 = 1.0;
fn scaled_size(size: f32) -> String {
    format!("{}px", size * TEXT_SCALE)
}

/// Creates an icon helper.
fn icon(icon: Icon) -> IconHelper {
    IconHelper::new(icon)
}

/// Helper for rendering icons with fluent styling.
struct IconHelper {
    icon: Icon,
    style: IconStyle,
}

impl IconHelper {
    pub fn new(icon: Icon) -> Self {
        Self {
            icon,
            style: IconStyle::new(16.0),
        }
    }

    pub fn custom_size(mut self, size: f32) -> Self {
        self.style.size = size;
        self
    }

    pub fn weight(mut self, weight: f32) -> Self {
        self.style.weight = Some(weight);
        self
    }

    /// Applies a font style so that the icon matches text sizing.
    pub fn font(mut self, font_style: FontStyle) -> Self {
        self.style.size = font_style.size;
        if let Some(icon_weight) = font_weight_to_icon_weight(font_style.weight) {
            self.style.weight = Some(icon_weight);
        }
        self
    }

    pub fn large_title(self) -> Dom {
        self.font(FontStyle::new(34.0, 41.0).weight("bold"))
            .finish()
    }

    pub fn title(self) -> Dom {
        self.font(FontStyle::new(28.0, 34.0).weight("bold"))
            .finish()
    }

    pub fn title2(self) -> Dom {
        self.font(FontStyle::new(22.0, 28.0).weight("bold"))
            .finish()
    }

    pub fn title3(self) -> Dom {
        self.font(FontStyle::new(20.0, 25.0).weight("bold"))
            .finish()
    }

    pub fn headline(self) -> Dom {
        self.font(FontStyle::new(17.0, 22.0).weight("600")).finish()
    }

    pub fn body(self) -> Dom {
        self.font(FontStyle::new(17.0, 22.0)).finish()
    }

    pub fn callout(self) -> Dom {
        self.font(FontStyle::new(16.0, 21.0).italic()).finish()
    }

    pub fn subheadline(self) -> Dom {
        self.font(FontStyle::new(15.0, 20.0)).finish()
    }

    pub fn footnote(self) -> Dom {
        self.font(FontStyle::new(13.0, 18.0)).finish()
    }

    pub fn caption(self) -> Dom {
        self.font(FontStyle::new(12.0, 16.0)).finish()
    }

    pub fn caption2(self) -> Dom {
        self.font(FontStyle::new(11.0, 13.0)).finish()
    }

    pub fn custom(self, font_style: FontStyle) -> Dom {
        self.font(font_style).finish()
    }

    /// Finalizes the icon rendering.
    fn finish(self) -> Dom {
        html!("div", {
            .class(&*STANDARD_FONT_CLASS)
            .style("display", "inline-block")
            .style("width", &scaled_size(self.style.size))
            .style("height", &scaled_size(self.style.size))
            .apply_if(self.style.weight.is_some(), |element| {
                element.style("--icon-weight", &format!("{}px", self.style.weight.unwrap()))
            })
            .child(render_icon_svg(self.icon))
        })
    }
}

/// Example function that renders various icon sizes.
fn icon_test() -> Dom {
    html!("div", {
        .children(&mut [
            icon(Icon::Plus).large_title(),
            icon(Icon::Plus).title(),
            icon(Icon::Plus).title2(),
            icon(Icon::Plus).title3(),
            icon(Icon::Plus).headline(),
            icon(Icon::Plus).body(),
            icon(Icon::Plus).callout(),
            icon(Icon::Plus).subheadline(),
            icon(Icon::Plus).footnote(),
            icon(Icon::Plus).caption(),
            icon(Icon::Plus).caption2(),
            icon(Icon::Plus).custom(FontStyle::new(18.0, 24.0).weight("500").italic()),
        ])
    })
}

/// Example function that renders text in various styles.
fn text_test() -> Dom {
    html!("div", {
        .children(&mut [
            text("Large Title").large_title(),
            text("Title").title(),
            text("Title 2").title2(),
            text("Title 3").title3(),
            text("Headline").headline(),
            text("Body").body(),
            text("Callout").callout(),
            text("Subheadline").subheadline(),
            text("Footnote").footnote(),
            text("Caption").caption(),
            text("Caption2").caption2(),
            text("Custom Text").custom(FontStyle::new(18.0, 24.0).weight("500").italic()),
        ])
    })
}

struct App {}

impl App {
    fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Renders the full app with header, tab bar, and tab content.
    fn render(state: &Arc<Self>) -> Dom {
        html!("div", {
            .style("padding", "1rem")
            .children(&mut [
                text("text").body(),
            ])
        })
    }
}

/// The entry point. We set up better panic messages in debug mode,
/// create our App instance, render it, and append it to the document body.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // Enable better panic messages in debug mode.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    let app = App::new();
    dominator::append_dom(&dominator::body(), App::render(&app));

    Ok(())
}
