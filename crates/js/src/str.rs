//! A reference-counted string behind one pointer: the length lives in the
//! allocation, so a value holding it stays two words wide.

use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::borrow::Borrow;
use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::ptr::NonNull;

#[repr(C)]
struct Head {
    rc: Cell<usize>,
    len: usize,
}

const HEAD: usize = size_of::<Head>();

pub struct Str(NonNull<Head>);

impl Str {
    fn layout(len: usize) -> Layout {
        Layout::from_size_align(HEAD + len, align_of::<Head>()).expect("string too long")
    }

    pub fn new(s: &str) -> Str {
        let layout = Self::layout(s.len());
        // SAFETY: the layout is non-zero (it holds the header); the header is
        // written before use and the bytes copied fit the allocation.
        unsafe {
            let p = alloc(layout) as *mut Head;
            if p.is_null() {
                handle_alloc_error(layout);
            }
            p.write(Head {
                rc: Cell::new(1),
                len: s.len(),
            });
            std::ptr::copy_nonoverlapping(s.as_ptr(), (p as *mut u8).add(HEAD), s.len());
            Str(NonNull::new_unchecked(p))
        }
    }

    #[inline]
    fn head(&self) -> &Head {
        // SAFETY: the allocation lives while any handle does.
        unsafe { self.0.as_ref() }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: the bytes after the header were copied from a `str`.
        unsafe {
            let bytes = std::slice::from_raw_parts(
                (self.0.as_ptr() as *const u8).add(HEAD),
                self.head().len,
            );
            std::str::from_utf8_unchecked(bytes)
        }
    }

    #[inline]
    pub fn strong_count(this: &Str) -> usize {
        this.head().rc.get()
    }

    #[inline]
    pub fn ptr_eq(a: &Str, b: &Str) -> bool {
        a.0 == b.0
    }
}

impl Clone for Str {
    #[inline]
    fn clone(&self) -> Str {
        let rc = &self.head().rc;
        rc.set(rc.get() + 1);
        Str(self.0)
    }
}

impl Drop for Str {
    #[inline]
    fn drop(&mut self) {
        let rc = &self.head().rc;
        let n = rc.get() - 1;
        rc.set(n);
        if n == 0 {
            let layout = Self::layout(self.head().len);
            // SAFETY: the last handle frees the allocation made in `new`.
            unsafe { dealloc(self.0.as_ptr() as *mut u8, layout) }
        }
    }
}

impl Deref for Str {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &str {
        self
    }
}

impl Borrow<str> for Str {
    fn borrow(&self) -> &str {
        self
    }
}

impl PartialEq for Str {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        self.0 == other.0 || self.as_str() == other.as_str()
    }
}

impl Eq for Str {}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Str {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialOrd for Str {
    fn partial_cmp(&self, other: &Str) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Str {
    fn cmp(&self, other: &Str) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Str {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_str().hash(h)
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self)
    }
}

impl Default for Str {
    fn default() -> Str {
        Str::new("")
    }
}

impl From<&str> for Str {
    fn from(s: &str) -> Str {
        Str::new(s)
    }
}

impl From<String> for Str {
    fn from(s: String) -> Str {
        Str::new(&s)
    }
}

impl From<&String> for Str {
    fn from(s: &String) -> Str {
        Str::new(s)
    }
}

impl From<Box<str>> for Str {
    fn from(s: Box<str>) -> Str {
        Str::new(&s)
    }
}

impl From<char> for Str {
    fn from(c: char) -> Str {
        Str::new(c.encode_utf8(&mut [0; 4]))
    }
}

impl From<&Str> for String {
    fn from(s: &Str) -> String {
        s.as_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::Str;

    #[test]
    fn round_trip_and_sharing() {
        let a = Str::from("hello");
        let b = a.clone();
        assert_eq!(Str::strong_count(&a), 2);
        assert!(Str::ptr_eq(&a, &b));
        drop(b);
        assert_eq!(&*a, "hello");
        assert_eq!(Str::from(""), Str::default());
        assert_eq!(size_of::<Str>(), 8);
        assert_eq!(size_of::<Option<Str>>(), 8);
    }
}
