
//! A no-std able crate to handle scopes (like xml/html) that should be closed when writing, 
//! with no allocation.

#![cfg_attr(not(feature = "std"), no_std)]

use core::{
    fmt::{self, Display},
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr,
};

#[cfg(feature = "std")]
mod with_std;
#[cfg(feature = "std")]
pub use with_std::WrapIO;

pub struct WrapFmt<W>(W);

impl<W: core::fmt::Write> Open for WrapFmt<W> {
    type Error = core::fmt::Error;

    type W = Self;

    fn writer(&mut self) -> &mut Self::W {
        self
    }

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        core::fmt::Write::write_fmt(&mut self.0, arg)
    }
}

impl<T: Open> Open for &mut T {
    type Error = T::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        (*self).write_fmt(arg)
    }

    type W = T;

    fn writer(&mut self) -> &mut Self::W {
        self
    }
}

pub trait Closer {
    fn remove_from<W: Open>(&mut self, w: W) -> Result<(), W::Error>;
}

pub trait Opener {
    type Close: Closer;
    fn add_to<W: Open>(self, w: W) -> Result<Self::Close, W::Error>;
}

pub trait ConstantCloseOpener {
    fn add_to<W: Open>(self, w: W) -> Result<(), W::Error>;
    fn remove_from<W: Open>(w: W) -> Result<(), W::Error>;
}

pub struct ConstantCloser<O>(
    // Variance baby
    PhantomData<fn(O)>,
);

impl<O: ConstantCloseOpener> Closer for ConstantCloser<O> {
    fn remove_from<W: Open>(&mut self, w: W) -> Result<(), W::Error> {
        O::remove_from(w)
    }
}

impl<O: ConstantCloseOpener> Opener for O {
    type Close = ConstantCloser<O>;

    fn add_to<W: Open>(self, w: W) -> Result<Self::Close, W::Error> {
        self.add_to(w)?;
        Ok(ConstantCloser(PhantomData))
    }
}

pub struct DynConstantCloser<W: Open> {
    closer: fn(&mut W) -> Result<(), W::Error>,
    phantom: PhantomData<fn(W)>,
}

impl<W: Open> DynConstantCloser<W> {
    pub fn new<O: ConstantCloseOpener>() -> Self {
        Self {
            closer: |w| O::remove_from(w),
            phantom: PhantomData,
        }
    }
}

pub trait CloserDynComp<W: Open> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error>;
}

impl<W: Open, C: Closer> CloserDynComp<W> for C {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        self.remove_from(w)
    }
}

impl<W: Open> CloserDynComp<W> for DynConstantCloser<W> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        (self.closer)(w)
    }
}

pub struct WriteScope<E: CloserDynComp<W>, W: Open> {
    closer: E,
    writer: W,
}

impl<E: Closer, W: Open> Deref for WriteScope<E, W> {
    type Target = W;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl<E: CloserDynComp<W>, W: Open> Drop for WriteScope<E, W> {
    fn drop(&mut self) {
        let do_drop: bool = {
            #[cfg(feature = "std")]
            {
                !std::thread::panicking()
            }
            #[cfg(not(feature = "std"))]
            {
                true
            }
        };
        if do_drop {
            self.closer.remove_from_dyn_comp(&mut self.writer).unwrap()
        }
    }
}

impl<E: Closer, W: Open> DerefMut for WriteScope<E, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

impl<C: Closer, W: Open> WriteScope<C, W> {
    fn deconstruct(self) -> (C, W) {
        let slf = ManuallyDrop::new(self);
        let w = unsafe { ptr::read(&slf.writer) };
        let c = unsafe { ptr::read(&slf.closer) };
        (c, w)
    }

    pub fn close(self) -> Result<W, W::Error> {
        let (mut closer, mut w) = self.deconstruct();
        closer.remove_from_dyn_comp(&mut w)?;
        Ok(w)
    }
}

impl<O: ConstantCloseOpener, W: Open> WriteScope<ConstantCloser<O>, W> {
    pub fn type_erase(self) -> WriteScope<DynConstantCloser<W>, W> {
        let (_clos, writer) = self.deconstruct();
        WriteScope {
            closer: DynConstantCloser::new::<O>(),
            writer,
        }
    }
}

/// The work-horse of this crate.
///
/// This is a kind of hybrid between DerefMut where Target: Write,
/// and Write itself, with added methods with default implementations to open scopes.
/// This is done in one big traits rather than with super traits to avoid issues with the orphan rule.
pub trait Open {
    /// The underlying writer's type
    type W: Open<Error = Self::Error>;

    /// The Error that happens when writing
    type Error: Display + fmt::Debug;

    /// The underlying writer
    fn writer(&mut self) -> &mut Self::W;

    /// Write-like interface allowing to be used with the [write!] macro
    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error>;

    /// Open a new scope, taking a reference to self
    fn ref_open<O: Opener>(
        &mut self,
        e: O,
    ) -> Result<WriteScope<O::Close, &mut Self::W>, <Self::W as Open>::Error> {
        let closer = e.add_to(self.writer())?;
        Ok(WriteScope {
            closer,
            writer: self.writer(),
        })
    }

    /// Open a scope and pass it as argument to the provided closure, closing it at the closures end
    fn open_scope<O: Opener>(
        &mut self,
        e: O,
        f: impl FnOnce(&mut WriteScope<O::Close, &mut Self::W>) -> Result<(), <Self::W as Open>::Error>,
    ) -> Result<(), <Self::W as Open>::Error> {
        let mut scope = self.ref_open(e)?;
        f(&mut scope)?;
        scope.close()?;
        Ok(())
    }

    /// Open a new scope taking self by ownership
    fn open<O: Opener>(
        mut self,
        e: O,
    ) -> Result<WriteScope<O::Close, Self>, <Self::W as Open>::Error>
    where
        Self: Sized,
    {
        let closer = e.add_to(self.writer())?;
        Ok(WriteScope {
            closer,
            writer: self,
        })
    }

    /// Write the argument to the writer
    fn text(&mut self, d: impl Display) -> Result<(), <Self::W as Open>::Error> {
        write!(self.writer(), "{d}")
    }
}

impl<E: Closer, W: Open> Open for WriteScope<E, W> {
    type W = W;

    fn writer(&mut self) -> &mut Self::W {
        &mut self.writer
    }

    type Error = W::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        self.writer.write_fmt(arg)
    }
}

pub mod xml;
