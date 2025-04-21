use crate::{ConstantCloseOpener, Write};

pub trait Element: Sized {
    const TAG: &str;

    fn tag_and_attributes<W: Write>(self, w: W) -> Result<(), W::Error>;
}

impl<E: Element> ConstantCloseOpener for E {
    fn add_to<W: Write>(self, mut w: W) -> Result<(), W::Error> {
        write!(&mut w, "<")?;
        self.tag_and_attributes(&mut w)?;
        writeln!(&mut w, ">")?;
        Ok(())
    }

    fn remove_from<W: Write>(mut w: W) -> Result<(), W::Error> {
        writeln!(w, "</{}>", <Self as Element>::TAG)
    }
}

pub trait SimpleElement {
    const TAG: &str;
}

impl<E: SimpleElement> Element for E {
    const TAG: &str = Self::TAG;

    fn tag_and_attributes<W: Write>(self, mut w: W) -> Result<(), W::Error> {
        write!(w, "{}", Self::TAG)
    }
}
