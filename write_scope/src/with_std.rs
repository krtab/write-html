use core::fmt;

use crate::{Closer, CloserDynComp, Open, WriteScope};

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


impl<W: Open> CloserDynComp<W> for Box<dyn CloserDynComp<W>> {
    fn remove_from_dyn_comp(&mut self, w: &mut W) -> Result<(), W::Error> {
        (**self).remove_from_dyn_comp(w)
    }
}

impl<C: Closer, W: Open> WriteScope<C, W> {
    pub fn closer_box_dyn(self) -> WriteScope<Box<dyn CloserDynComp<W>>, W>
    where
        C: 'static,
    {
        let (closer, writer) = self.deconstruct();
        WriteScope {
            closer: Box::new(closer),
            writer,
        }
    }
}