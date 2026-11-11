//! The browser, typed: the subset of its APIs a managed module (`.apeiron`)
//! may use, written by hand ([decision 0009]). Each type, member and global
//! is listed here once; the checker types them, the JavaScript generator
//! writes them, and the reference page lists them, checked by the tests.
//!
//! Three of these types are resources: a `Listener`, a `Timer` and a
//! `Group` of them belong to one variable, and are released when it goes out
//! of scope, unless they are given away first.
//!
//! [decision 0009]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0009-browser-and-resources.md

use crate::ty::{FloatTy, IntTy, Ty};

/// A type of the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Host {
    /// The page: `document`.
    Document,
    /// An element of the page.
    Element,
    /// An `<input>` element.
    Input,
    /// A `<canvas>` element.
    Canvas,
    /// A canvas's 2D drawing context.
    Context2d,
    /// An event given to a listener.
    Event,
    /// The answer to `fetch`.
    Response,
    /// The page's `localStorage`: `storage`.
    Storage,
    /// A resource: an event listener, removed when released.
    Listener,
    /// A resource: a timeout or an interval, cleared when released.
    Timer,
    /// A resource that holds others, and releases them with it.
    Group,
    /// A place of a group that holds one resource at a time: `group.slot()`.
    /// Not a resource itself: the group owns what it holds.
    Slot,
}

impl Host {
    /// Every type, in the order of the reference.
    pub const ALL: [Self; 12] = [
        Self::Document,
        Self::Element,
        Self::Input,
        Self::Canvas,
        Self::Context2d,
        Self::Event,
        Self::Response,
        Self::Storage,
        Self::Listener,
        Self::Timer,
        Self::Group,
        Self::Slot,
    ];

    /// Its name in Peras.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Element => "Element",
            Self::Input => "InputElement",
            Self::Canvas => "CanvasElement",
            Self::Context2d => "Context2d",
            Self::Event => "Event",
            Self::Response => "Response",
            Self::Storage => "Storage",
            Self::Listener => "Listener",
            Self::Timer => "Timer",
            Self::Group => "Group",
            Self::Slot => "Slot",
        }
    }

    /// Its name in TypeScript's declarations of the DOM.
    #[must_use]
    pub const fn ts_name(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Element => "Element",
            Self::Input => "HTMLInputElement",
            Self::Canvas => "HTMLCanvasElement",
            Self::Context2d => "CanvasRenderingContext2D",
            Self::Event => "Event",
            Self::Response => "Response",
            Self::Storage => "Storage",
            Self::Listener | Self::Timer | Self::Group => "PerasResource",
            Self::Slot => "PerasSlot",
        }
    }

    /// Whether it is a resource: owned by one variable, released with it.
    #[must_use]
    pub const fn is_resource(self) -> bool {
        matches!(self, Self::Listener | Self::Timer | Self::Group)
    }

    /// Whether it takes listeners with `on(event, handler)`.
    #[must_use]
    pub const fn has_events(self) -> bool {
        matches!(self, Self::Element | Self::Input | Self::Canvas)
    }
}

/// A member of a browser type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    /// A property: its type, and whether it can be assigned.
    Field(Ty, bool),
    /// A method: its parameters and its result.
    Method(Vec<Ty>, Ty),
}

fn host(h: Host) -> Ty {
    Ty::Host(h)
}

fn option(t: Ty) -> Ty {
    Ty::Option(Box::new(t))
}

const fn f64() -> Ty {
    Ty::Float(FloatTy::F64)
}

const fn i32() -> Ty {
    Ty::Int(IntTy::I32)
}

/// The members of each type: name, member, and how JavaScript writes it.
#[must_use]
pub fn members(of: Host) -> Vec<(&'static str, Member)> {
    use Member::{Field, Method};
    let s = || Ty::String;
    match of {
        Host::Document => vec![
            ("body", Field(host(Host::Element), false)),
            ("title", Field(s(), true)),
            (
                "getElementById",
                Method(vec![s()], option(host(Host::Element))),
            ),
            (
                "querySelector",
                Method(vec![s()], option(host(Host::Element))),
            ),
            ("createElement", Method(vec![s()], host(Host::Element))),
        ],
        Host::Element => vec![
            ("textContent", Field(s(), true)),
            ("id", Field(s(), true)),
            (
                "querySelector",
                Method(vec![s()], option(host(Host::Element))),
            ),
            ("getAttribute", Method(vec![s()], option(s()))),
            ("setAttribute", Method(vec![s(), s()], Ty::Void)),
            ("append", Method(vec![host(Host::Element)], Ty::Void)),
            ("remove", Method(Vec::new(), Ty::Void)),
            ("addClass", Method(vec![s()], Ty::Void)),
            ("removeClass", Method(vec![s()], Ty::Void)),
            ("toggleClass", Method(vec![s()], Ty::Bool)),
            ("focus", Method(Vec::new(), Ty::Void)),
            ("click", Method(Vec::new(), Ty::Void)),
            ("asInput", Method(Vec::new(), option(host(Host::Input)))),
            ("asCanvas", Method(Vec::new(), option(host(Host::Canvas)))),
        ],
        Host::Input => vec![
            ("value", Field(s(), true)),
            ("focus", Method(Vec::new(), Ty::Void)),
        ],
        Host::Canvas => vec![
            ("width", Field(i32(), true)),
            ("height", Field(i32(), true)),
            (
                "context2d",
                Method(Vec::new(), option(host(Host::Context2d))),
            ),
        ],
        Host::Context2d => {
            let four = || vec![f64(), f64(), f64(), f64()];
            vec![
                ("fillStyle", Field(s(), true)),
                ("strokeStyle", Field(s(), true)),
                ("lineWidth", Field(f64(), true)),
                ("font", Field(s(), true)),
                ("fillRect", Method(four(), Ty::Void)),
                ("strokeRect", Method(four(), Ty::Void)),
                ("clearRect", Method(four(), Ty::Void)),
                ("fillText", Method(vec![s(), f64(), f64()], Ty::Void)),
                ("beginPath", Method(Vec::new(), Ty::Void)),
                ("moveTo", Method(vec![f64(), f64()], Ty::Void)),
                ("lineTo", Method(vec![f64(), f64()], Ty::Void)),
                (
                    "arc",
                    Method(vec![f64(), f64(), f64(), f64(), f64()], Ty::Void),
                ),
                ("fill", Method(Vec::new(), Ty::Void)),
                ("stroke", Method(Vec::new(), Ty::Void)),
            ]
        }
        Host::Event => vec![
            // `type` is a keyword: JavaScript's `event.type` is `kind`.
            ("kind", Field(s(), false)),
            ("key", Field(option(s()), false)),
            ("clientX", Field(option(f64()), false)),
            ("clientY", Field(option(f64()), false)),
            ("preventDefault", Method(Vec::new(), Ty::Void)),
            ("stopPropagation", Method(Vec::new(), Ty::Void)),
        ],
        Host::Response => vec![
            ("ok", Field(Ty::Bool, false)),
            ("status", Field(i32(), false)),
            ("text", Method(Vec::new(), Ty::Promise(Box::new(s())))),
        ],
        Host::Storage => vec![
            ("get", Method(vec![s()], option(s()))),
            ("set", Method(vec![s(), s()], Ty::Void)),
            ("remove", Method(vec![s()], Ty::Void)),
        ],
        Host::Listener | Host::Timer => vec![("release", Method(Vec::new(), Ty::Void))],
        Host::Group => vec![
            // `add` takes any resource: checked apart.
            ("add", Method(vec![Ty::Error], Ty::Void)),
            ("slot", Method(Vec::new(), Ty::Host(Host::Slot))),
            ("release", Method(Vec::new(), Ty::Void)),
        ],
        Host::Slot => vec![
            // `set` takes any resource, and releases the one it replaces.
            ("set", Method(vec![Ty::Error], Ty::Void)),
            ("clear", Method(Vec::new(), Ty::Void)),
        ],
    }
}

/// A member of a type, by name.
#[must_use]
pub fn member(of: Host, name: &str) -> Option<Member> {
    members(of)
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, m)| m)
}

/// A global of the browser: a value, or a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Global {
    /// `document`, `storage`.
    Value(Ty),
    /// `fetch`, `sleep`, `now`, `log`: parameters and result.
    Function(Vec<Ty>, Ty),
    /// `setTimeout`, `setInterval`: a delay in milliseconds and an arrow
    /// function; gives a `Timer`.
    Timer,
}

/// Every global, in the order of the reference.
pub const GLOBALS: [&str; 8] = [
    "document",
    "storage",
    "fetch",
    "sleep",
    "now",
    "log",
    "setTimeout",
    "setInterval",
];

/// A global, by name.
#[must_use]
pub fn global(name: &str) -> Option<Global> {
    Some(match name {
        "document" => Global::Value(host(Host::Document)),
        "storage" => Global::Value(host(Host::Storage)),
        "fetch" => Global::Function(
            vec![Ty::String],
            Ty::Promise(Box::new(host(Host::Response))),
        ),
        "sleep" => Global::Function(vec![i32()], Ty::Promise(Box::new(Ty::Void))),
        "now" => Global::Function(Vec::new(), f64()),
        "log" => Global::Function(vec![Ty::String], Ty::Void),
        "setTimeout" | "setInterval" => Global::Timer,
        _ => return None,
    })
}

/// Whether a type is, or holds, a resource.
#[must_use]
pub fn holds_resource(ty: &Ty) -> bool {
    match ty {
        Ty::Host(h) => h.is_resource(),
        Ty::Array(t) | Ty::Option(t) | Ty::Set(t) | Ty::Promise(t) => holds_resource(t),
        Ty::Map(a, b) | Ty::Result(a, b) => holds_resource(a) || holds_resource(b),
        _ => false,
    }
}

/// Whether a type is a resource itself.
#[must_use]
pub const fn is_resource(ty: &Ty) -> bool {
    matches!(ty, Ty::Host(h) if h.is_resource())
}
