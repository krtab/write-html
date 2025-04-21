use std::{
    fmt::{self, Display},
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr,
};

pub trait Write {
    type Error: Display + fmt::Debug;
    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error>;
}

pub struct WrapFMT<W>(pub W);

impl<W: std::fmt::Write> Write for WrapFMT<W> {
    type Error = std::fmt::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        std::fmt::Write::write_fmt(&mut self.0, arg)
    }
}


pub struct WrapIO<W>(pub W);

impl<W: std::io::Write> Write for WrapIO<W> {
    type Error = std::io::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        std::io::Write::write_fmt(&mut self.0, arg)
    }
}



impl<T: Write> Write for &mut T {
    type Error = T::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        (*self).write_fmt(arg)
    }
}

pub trait CloserDynComp<W: Write> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error>;
}

impl<W: Write> CloserDynComp<W> for Box<dyn CloserDynComp<W>> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        (**self).remove_from_dyn_comp(w)
    }
}

pub trait Closer {
    fn remove_from<W: Write>(&mut self, w: W) -> Result<(), W::Error>;

    fn as_boxdyn<W: Write>(self) -> Box<dyn CloserDynComp<W>>
    where
        Self: Sized,
        Self: 'static,
    {
        Box::new(self)
    }
}

impl<W: Write, C: Closer> CloserDynComp<W> for C {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        self.remove_from(w)
    }
}

pub trait Opener {
    type Close: Closer;
    fn add_to<W: Write>(self, w: W) -> Result<Self::Close, W::Error>;
}

pub trait ConstantCloseOpener {
    fn add_to<W: Write>(self, w: W) -> Result<(), W::Error>;
    fn remove_from<W: Write>(w: W) -> Result<(), W::Error>;
}

pub struct ConstantCloser<O>(
    // Variance baby
    PhantomData<fn(O)>,
);

impl<O: ConstantCloseOpener> Closer for ConstantCloser<O> {
    fn remove_from<W: Write>(&mut self, w: W) -> Result<(), W::Error> {
        O::remove_from(w)
    }
}

impl<O: ConstantCloseOpener> Opener for O {
    type Close = ConstantCloser<O>;

    fn add_to<W: Write>(self, w: W) -> Result<Self::Close, W::Error> {
        self.add_to(w)?;
        Ok(ConstantCloser(PhantomData))
    }
}

pub struct DynConstantCloser<W: Write> {
    closer: fn(&mut W) -> Result<(), W::Error>,
    phantom: PhantomData<fn(W)>,
}

impl<W: Write> DynConstantCloser<W> {
    pub fn new<O: ConstantCloseOpener>() -> Self {
        Self {
            closer: |w| O::remove_from(w),
            phantom: PhantomData,
        }
    }
}

impl<W: Write> CloserDynComp<W> for DynConstantCloser<W> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        (self.closer)(w)
    }
}

pub struct WriteScope<E: CloserDynComp<W>, W: Write> {
    closer: E,
    writer: W,
}

impl<E: Closer, W: Write> Deref for WriteScope<E, W> {
    type Target = W;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl<E: CloserDynComp<W>, W: Write> Drop for WriteScope<E, W> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.closer.remove_from_dyn_comp(&mut self.writer).unwrap()
        }
    }
}

impl<E: Closer, W: Write> DerefMut for WriteScope<E, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

pub struct Wrap<O>(O);

impl<O: Open> Write for Wrap<O> {
    type Error = <<O as Open>::W as Write>::Error;

    fn write_fmt(&mut self, arg: fmt::Arguments) -> Result<(), Self::Error> {
        self.0.writer().write_fmt(arg)
    }
}

impl<C: Closer, W: Write> WriteScope<C, W> {
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

impl<O: ConstantCloseOpener, W: Write> WriteScope<ConstantCloser<O>, W> {
    pub fn type_erase(self) -> WriteScope<DynConstantCloser<W>, W> {
        let (_clos, writer) = self.deconstruct();
        WriteScope {
            closer: DynConstantCloser::new::<O>(),
            writer,
        }
    }
}

pub trait Open {
    type W: Write;

    fn writer(&mut self) -> &mut Self::W;

    fn ref_open<O: Opener>(
        &mut self,
        e: O,
    ) -> Result<WriteScope<O::Close, &mut Self::W>, <Self::W as Write>::Error> {
        let closer = e.add_to(self.writer())?;
        Ok(WriteScope {
            closer,
            writer: self.writer(),
        })
    }

    fn open_scope<O: Opener>(
        &mut self,
        e: O,
        f: impl FnOnce(&mut WriteScope<O::Close, &mut Self::W>) -> Result<(), <Self::W as Write>::Error>,
    ) -> Result<(), <Self::W as Write>::Error> {
        let mut scope = self.ref_open(e)?;
        f(&mut scope)?;
        scope.close()?;
        Ok(())
    }

    fn open<O: Opener>(
        mut self,
        e: O,
    ) -> Result<WriteScope<O::Close, Wrap<Self>>, <Self::W as Write>::Error>
    where
        Self: Sized,
        Wrap<Self>: Write,
    {
        let closer = e.add_to(self.writer())?;
        Ok(WriteScope {
            closer,
            writer: Wrap(self),
        })
    }

    fn text(&mut self, d: impl Display) -> Result<(), <Self::W as Write>::Error> {
        write!(self.writer(), "{d}")
    }

    fn wrap(self) -> Wrap<Self>
    where
        Self: Sized,
    {
        Wrap(self)
    }
}

impl<W: Write> Open for W {
    type W = Self;

    fn writer(&mut self) -> &mut Self::W {
        self
    }
}

impl<E: Closer, W: Write> Open for WriteScope<E, W> {
    type W = W;

    fn writer(&mut self) -> &mut Self::W {
        &mut self.writer
    }
}

pub mod xml;
