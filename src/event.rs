use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

pub trait EventTargetExt {
    fn add_event_listener<E: Event>(&self, f: impl FnMut(E) + 'static) -> ListenerHandle;
}

impl EventTargetExt for web_sys::EventTarget {
    fn add_event_listener<E: Event>(&self, mut f: impl FnMut(E) + 'static) -> ListenerHandle {
        let closure = Closure::wrap(Box::new(move |e| {
            f(E::from_event(e));
        }) as Box<dyn FnMut(web_sys::Event)>);
        self.add_event_listener_with_callback(E::NAME, closure.as_ref().unchecked_ref())
            .unwrap();
        ListenerHandle {
            target: self.clone(),
            name: E::NAME,
            closure: Some(closure),
        }
    }
}

/// Handle to an event listener callback.
///
/// When dropped, this removes the callback from the event target.
pub struct ListenerHandle {
    target: web_sys::EventTarget,
    name: &'static str,
    closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl ListenerHandle {
    /// Leak the event listener so it lives forever.
    pub fn forget(mut self) {
        self.closure.take().unwrap().forget();
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        if let Some(c) = &self.closure {
            self.target
                .remove_event_listener_with_callback(self.name, c.as_ref().unchecked_ref())
                .unwrap();
        }
    }
}

pub trait Event {
    const NAME: &'static str;
    fn from_event(e: web_sys::Event) -> Self;
}

macro_rules! event {
    ($($type:ident $raw:ident $name:tt;)*) => {
        $(
            pub struct $type(web_sys::$raw);

            impl Event for $type {
                const NAME: &'static str = $name;

                fn from_event(e: web_sys::Event) -> Self {
                    $type(e.unchecked_into())
                }
            }

            impl std::ops::Deref for $type {
                type Target = web_sys::$raw;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        )*
    };
}

event! {
    // Resource events
    Error        Event "error";
    Abort        Event "abort";
    Load         Event "load";
    BeforeUnload Event "beforeunload";
    Unload       Event "unload";

    // Network events
    Online  Event "online";
    Offline Event "offline";

    // Focus events
    Focus    FocusEvent "focus";
    Blur     FocusEvent "blur";
    FocusIn  FocusEvent "focusin";
    FocusOut FocusEvent "focusout";

    // WebSocket events
    Open    Event        "open";
    Message MessageEvent "message";
    Close   CloseEvent   "close";

    // Session History events
    PageHide PageTransitionEvent "pagehide";
    PageShow PageTransitionEvent "pageshow";
    PopState PopStateEvent       "popstate";

    // Form events
    Reset  Event "reset";
    Submit Event "submit"; // should be a SubmitEvent but that doesn't seem to be in web-sys?

    // View events
    FullscreenChange Event   "fullscreenchange";
    FullscreenError  Event   "fullscreenerror";
    Resize           UiEvent "resize";
    Scroll           Event   "scroll";

    // Clipboard events
    Cut    ClipboardEvent "cut";
    Copied ClipboardEvent "copy";
    Paste  ClipboardEvent "paste";

    // Keyboard events
    KeyDown    KeyboardEvent "keydown";
    KeyUp      KeyboardEvent "keyup";
    KeyPressed KeyboardEvent "keypress";

    // Mouse events
    AuxClick          MouseEvent "auxclick";
    Click             MouseEvent "click";
    ContextMenu       MouseEvent "contextmenu";
    DoubleClick       MouseEvent "dblclick";
    MouseDown         MouseEvent "mousedown";
    MouseEnter        MouseEvent "mouseenter";
    MouseLeave        MouseEvent "mouseleave";
    MouseMove         MouseEvent "mousemove";
    MouseOver         MouseEvent "mouseover";
    MouseOut          MouseEvent "mouseout";
    MouseUp           MouseEvent "mouseup";
    PointerLockChange Event      "pointerlockchange";
    PointerLockError  Event      "pointerlockerror";
    Select            Event      "select";
    Wheel             WheelEvent "onwheel";

    // TODO Drag and Drop events
    // TODO Media events
    // TODO Progress events
    // TODO Storage events
    // TODO Update events
    // TODO Value change events
    // TODO Uncategorized events
    // TODO Abortable Fetch events
    // TODO WebVR events
    // TODO SVG events
    // TODO Database events
    // TODO Tab events
    // TODO Sensor events
    // TODO Smartcard events
    // TODO DOM mutation events
    // TODO Touch events
    // TODO Pointer events
    // TODO Printing events
    // TODO Text Composition events
    // TODO CSS Animation events
    // TODO CSS Transition events
    // TODO Gamepad events
}
