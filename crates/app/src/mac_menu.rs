//! The app's menus ([`crate::app_menu::MENUS`]) in the macOS menu bar:
//! Settings… (⌘,) and Check for Updates Automatically in the app menu,
//! then File, Edit, View and Help after it.
//! Slint builds the app menu itself (About, Services, Hide, Quit) with no
//! way to add to it, so the items are added to the native menu once it
//! exists. Slint gives each window a menu bar of its own and swaps it in
//! when that window comes to the front, so [`install`] runs on a timer and
//! fills each new menu bar it finds.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventType, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSObject, NSString};

use crate::app_menu::{checked, shown, state, Action, Entry, MENUS};

thread_local! {
    /// What a picked item does, and the object the items call.
    static RUN: RefCell<Option<Box<dyn Fn(Action)>>> = RefCell::new(None);
    static TARGET: RefCell<Option<Retained<MenuTarget>>> = const { RefCell::new(None) };
    /// Every item, by what it does, to rename or grey out.
    static ITEMS: RefCell<Vec<(Action, Retained<NSMenuItem>)>> = const { RefCell::new(Vec::new()) };
    /// The menu bars already filled.
    static FILLED: RefCell<Vec<Retained<NSMenu>>> = const { RefCell::new(Vec::new()) };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class
    // doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DJSugarMenuTarget"]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(runMenuAction:))]
        fn run_menu_action(&self, sender: Option<&AnyObject>) {
            let Some(item) = sender.and_then(|s| s.downcast_ref::<NSMenuItem>()) else {
                return;
            };
            let Some(action) = Action::from_id(item.tag() as i32) else {
                return;
            };
            // The window may not see a shortcut the menu takes, so the
            // item does the work for it: Settings… only opens and Find only
            // goes to the search, so doing either twice is harmless. ⌘Z is
            // left to the window, which undoes there (or lets a text field
            // take it for its typing), so only a click on Undo counts.
            if action == Action::Undo {
                if let Some(mtm) = MainThreadMarker::new() {
                    let key = NSApplication::sharedApplication(mtm)
                        .currentEvent()
                        .is_some_and(|e| e.r#type() == NSEventType::KeyDown);
                    if key {
                        return;
                    }
                }
            }
            RUN.with(|r| {
                if let Some(run) = r.borrow().as_ref() {
                    run(action);
                }
            });
        }
    }
);

fn new_item(
    mtm: MainThreadMarker,
    target: &MenuTarget,
    action: Action,
    title: &str,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: the selector is one MenuTarget implements, and the target is
    // kept alive in TARGET for as long as the menu exists.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(sel!(runMenuAction:)),
            &NSString::from_str(key),
        )
    };
    unsafe { item.setTarget(Some(target)) };
    item.setTag(action.id() as isize);
    if let Some(on) = checked(action) {
        item.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
    ITEMS.with(|i| i.borrow_mut().push((action, item.clone())));
    item
}

fn fill(mtm: MainThreadMarker, target: &MenuTarget, menu: &NSMenu, entries: &'static [Entry]) {
    for e in shown(entries) {
        match e {
            Entry::Item { action, key, .. } => {
                // As things stand: Undo named for what it takes back, the
                // exports greyed out without a list to work on.
                let (title, enabled) = state(*action);
                let item = new_item(mtm, target, *action, &title, key.unwrap_or(""));
                item.setEnabled(enabled);
                menu.addItem(&item);
            }
            Entry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            Entry::Submenu { title, entries } => {
                let sub = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
                sub.setAutoenablesItems(false);
                fill(mtm, target, &sub, entries);
                let holder = NSMenuItem::new(mtm);
                holder.setTitle(&NSString::from_str(title));
                holder.setSubmenu(Some(&sub));
                menu.addItem(&holder);
            }
        }
    }
}

/// Sets what a picked item does. Call once, before [`install`].
pub fn on_action(run: impl Fn(Action) + 'static) {
    RUN.with(|r| *r.borrow_mut() = Some(Box::new(run)));
}

/// Adds Settings… under About in the app menu and the File, Edit… menus
/// after it, to the menu bar showing now, unless it has them already.
/// Cheap when there's nothing to do, so call it often: the menu bar only
/// exists once the event loop runs, and each window brings its own.
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(main) = app.mainMenu() else {
        return;
    };
    if FILLED.with(|f| {
        f.borrow()
            .iter()
            .any(|m| Retained::as_ptr(m) == Retained::as_ptr(&main))
    }) {
        return;
    }
    let Some(app_menu) = main.itemAtIndex(0).and_then(|i| i.submenu()) else {
        return;
    };
    let target = TARGET.with(|t| {
        t.borrow_mut()
            .get_or_insert_with(|| {
                let this = MenuTarget::alloc(mtm).set_ivars(());
                // SAFETY: NSObject's init, on a freshly allocated object.
                unsafe { msg_send![super(this), init] }
            })
            .clone()
    });
    // About, then a line, then Settings… and the update tick and a line, as
    // in other Mac apps.
    let settings = new_item(mtm, &target, Action::Settings, "Settings…", ",");
    let updates = new_item(
        mtm,
        &target,
        Action::CheckForUpdates,
        "Check for Updates Automatically",
        "",
    );
    let at = 2.min(app_menu.numberOfItems());
    app_menu.insertItem_atIndex(&settings, at);
    app_menu.insertItem_atIndex(&updates, at + 1);
    app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), at + 2);
    // File, Edit… after the app menu, in order.
    let mut at = 1.min(main.numberOfItems());
    for m in MENUS {
        if shown(m.entries).is_empty() {
            continue;
        }
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(m.title));
        menu.setAutoenablesItems(false);
        fill(mtm, &target, &menu, m.entries);
        let holder = NSMenuItem::new(mtm);
        holder.setSubmenu(Some(&menu));
        main.insertItem_atIndex(&holder, at);
        at += 1;
    }
    FILLED.with(|f| f.borrow_mut().push(main));
}

/// Renames an item and greys it out or back in.
pub fn set_item(action: Action, title: &str, enabled: bool) {
    ITEMS.with(|items| {
        for (a, item) in items.borrow().iter() {
            if *a == action {
                item.setTitle(&NSString::from_str(title));
                item.setEnabled(enabled);
            }
        }
    });
}

/// Ticks an item or takes its tick off.
pub fn set_checked(action: Action, on: bool) {
    ITEMS.with(|items| {
        for (a, item) in items.borrow().iter() {
            if *a == action {
                item.setState(if on {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                });
            }
        }
    });
}
