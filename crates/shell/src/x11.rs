use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, PropMode, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::{App, Error, Event, Frame};

pub fn run(title: &str, width: u32, height: u32, app: &mut impl App) -> Result<(), Error> {
    let (conn, screen) = RustConnection::connect(None).map_err(Error::from)?;
    let root = &conn.setup().roots[screen];
    let (root_window, depth) = (root.root, root.root_depth);
    let scale = dpi_scale(root.height_in_pixels, root.height_in_millimeters);
    let window = conn.generate_id().map_err(Error::from)?;
    let mask = EventMask::EXPOSURE
        | EventMask::STRUCTURE_NOTIFY
        | EventMask::KEY_PRESS
        | EventMask::KEY_RELEASE
        | EventMask::BUTTON_PRESS
        | EventMask::BUTTON_RELEASE
        | EventMask::POINTER_MOTION;
    conn.create_window(
        depth,
        window,
        root_window,
        0,
        0,
        width as u16,
        height as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().background_pixel(0).event_mask(mask),
    )
    .map_err(Error::from)?;
    let atom = |name: &[u8]| -> Result<u32, Error> {
        Ok(conn.intern_atom(false, name).map_err(Error::from)?.reply().map_err(Error::from)?.atom)
    };
    let (protocols, delete, net_name, utf8) =
        (atom(b"WM_PROTOCOLS")?, atom(b"WM_DELETE_WINDOW")?, atom(b"_NET_WM_NAME")?, atom(b"UTF8_STRING")?);
    conn.change_property32(PropMode::REPLACE, window, protocols, AtomEnum::ATOM, &[delete]).map_err(Error::from)?;
    conn.change_property8(PropMode::REPLACE, window, net_name, utf8, title.as_bytes()).map_err(Error::from)?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())
        .map_err(Error::from)?;
    let gc = conn.generate_id().map_err(Error::from)?;
    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0)).map_err(Error::from)?;
    conn.map_window(window).map_err(Error::from)?;
    conn.flush().map_err(Error::from)?;

    let setup = conn.setup();
    let (min_code, max_code) = (setup.min_keycode, setup.max_keycode);
    let map = conn
        .get_keyboard_mapping(min_code, max_code - min_code + 1)
        .map_err(Error::from)?
        .reply()
        .map_err(Error::from)?;
    let per = usize::from(map.keysyms_per_keycode);
    let keysym = |code: u8, shift: bool| -> u32 {
        let base = usize::from(code.saturating_sub(min_code)) * per;
        let syms = map.keysyms.get(base..base + per).unwrap_or(&[]);
        let lower = syms.first().copied().unwrap_or(0);
        match syms.get(1).copied() {
            Some(upper) if shift && upper != 0 => upper,
            _ if shift && (0x61..=0x7a).contains(&lower) => lower - 0x20,
            _ => lower,
        }
    };

    let max_request = conn.maximum_request_bytes();
    let mut size = (width, height);
    app.event(Event::Resize { width, height, scale });
    let mut dirty = true;
    loop {
        let mut event = if dirty || app.animating() {
            conn.poll_for_event().map_err(Error::from)?
        } else {
            Some(conn.wait_for_event().map_err(Error::from)?)
        };
        while let Some(e) = event {
            let mapped = match e {
                XEvent::Expose(e) if e.count == 0 => Some(Event::Redraw),
                XEvent::ConfigureNotify(e) => {
                    let s = (u32::from(e.width), u32::from(e.height));
                    (s != size).then(|| {
                        size = s;
                        Event::Resize { width: s.0, height: s.1, scale }
                    })
                }
                XEvent::MotionNotify(e) => Some(Event::Pointer { x: f32::from(e.event_x), y: f32::from(e.event_y) }),
                XEvent::ButtonPress(e) => Some(Event::Button { button: e.detail, pressed: true }),
                XEvent::ButtonRelease(e) => Some(Event::Button { button: e.detail, pressed: false }),
                XEvent::KeyPress(e) => {
                    let state = u16::from(e.state);
                    Some(Event::Key { sym: keysym(e.detail, state & 1 != 0), pressed: true, mods: state })
                }
                XEvent::KeyRelease(e) => {
                    Some(Event::Key { sym: keysym(e.detail, false), pressed: false, mods: u16::from(e.state) })
                }
                XEvent::ClientMessage(e) if e.data.as_data32()[0] == delete => Some(Event::Close),
                _ => None,
            };
            match mapped {
                Some(Event::Close) => {
                    app.event(Event::Close);
                    return Ok(());
                }
                Some(Event::Redraw) => dirty = true,
                Some(ev) => dirty |= app.event(ev),
                None => {}
            }
            event = conn.poll_for_event().map_err(Error::from)?;
        }
        let animating = app.animating();
        if dirty || animating {
            dirty = false;
            if animating {
                std::thread::sleep(std::time::Duration::from_millis(8));
            }
            let mut result = Ok(());
            app.draw(&mut |f: Frame<'_>| result = put(&conn, window, gc, depth, &f, max_request));
            result?;
            conn.flush().map_err(Error::from)?;
        }
    }
}

fn put(conn: &RustConnection, window: u32, gc: u32, depth: u8, f: &Frame<'_>, max: usize) -> Result<(), Error> {
    let width = (f.stride / 4) as u16;
    let rows = ((max - 64) / f.stride).max(1);
    let mut y = 0;
    while y < f.height as usize {
        let n = rows.min(f.height as usize - y);
        let data = &f.bytes[y * f.stride..(y + n) * f.stride];
        conn.put_image(ImageFormat::Z_PIXMAP, window, gc, width, n as u16, 0, y as i16, 0, depth, data)
            .map_err(Error::from)?;
        y += n;
    }
    Ok(())
}

fn dpi_scale(px: u16, mm: u16) -> f32 {
    if mm == 0 {
        return 1.0;
    }
    let dpi = f32::from(px) * 25.4 / f32::from(mm);
    ((dpi / 96.0 * 4.0).round() / 4.0).clamp(1.0, 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_snaps_to_quarters() {
        assert_eq!(dpi_scale(1080, 286), 1.0);
        assert_eq!(dpi_scale(2160, 286), 2.0);
        assert_eq!(dpi_scale(1080, 0), 1.0);
    }
}
