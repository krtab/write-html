use std::{
    fmt::{self, Display},
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr,
};

pub struct WrapFmt<W>(W);

impl<W: std::fmt::Write> Open for WrapFmt<W> {
    type Error = std::fmt::Error;

    type W = Self;

    fn writer(&mut self) -> &mut Self::W {
        self
    }

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        std::fmt::Write::write_fmt(&mut self.0, arg)
    }
}

pub struct WrapIO<W>(pub W);

impl<W: std::io::Write> Open for WrapIO<W> {
    type Error = std::io::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        std::io::Write::write_fmt(&mut self.0, arg)
    }

    type W = Self;

    fn writer(&mut self) -> &mut Self::W {
        self
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

pub trait CloserDynComp<W: Open> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error>;
}

impl<W: Open> CloserDynComp<W> for Box<dyn CloserDynComp<W>> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        (**self).remove_from_dyn_comp(w)
    }
}

pub trait Closer {
    fn remove_from<W: Open>(&mut self, w: W) -> Result<(), W::Error>;

    fn as_boxdyn<W: Open>(self) -> Box<dyn CloserDynComp<W>>
    where
        Self: Sized,
        Self: 'static,
    {
        Box::new(self)
    }
}

impl<W: Open, C: Closer> CloserDynComp<W> for C {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        self.remove_from(w)
    }
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
        if !std::thread::panicking() {
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

    pub fn closer_box_dyn(self) -> WriteScope<Box<dyn CloserDynComp<W>>, W>
    where
        C: 'static,
    {
        let (closer, writer) = self.deconstruct();
        WriteScope {
            closer: closer.as_boxdyn(),
            writer,
        }
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

pub trait Open {
    type W: Open<Error = Self::Error>;

    type Error: Display + fmt::Debug;

    fn writer(&mut self) -> &mut Self::W;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error>;

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
