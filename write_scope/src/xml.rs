use crate::{ConstantCloseOpener, Open};

pub trait Element: Sized {
    const TAG: &str;

    fn tag_and_attributes<W: Open>(self, w: W) -> Result<(), W::Error>;
}

impl<E: Element> ConstantCloseOpener for E {
    fn add_to<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        write!(&mut w, "<")?;
        self.tag_and_attributes(&mut w)?;
        writeln!(&mut w, ">")?;
        Ok(())
    }

    fn remove_from<W: Open>(mut w: W) -> Result<(), W::Error> {
        writeln!(w, "</{}>", <Self as Element>::TAG)
    }
}

pub trait SimpleElement {
    const TAG: &str;
}

impl<E: SimpleElement> Element for E {
    const TAG: &str = Self::TAG;

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        write!(w, "{}", Self::TAG)
    }
}
