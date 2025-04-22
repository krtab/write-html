use std::fmt::Display;

use write_scope::{
    xml::{self, Element},
    Open,
};

pub trait HtmlElement: xml::Element {
    fn attr<K, V>(self, key: K, val: V) -> Attr<K, V, Self> {
        Attr {
            key,
            val,
            inner: self,
        }
    }

    fn id<S>(self, id: S) -> Id<S, Self> {
        Id {
            val: id,
            inner: self,
        }
    }

    fn class<S>(self, class: S) -> Class<S, Self> {
        Class {
            val: class,
            inner: self,
        }
    }
}

impl<E: xml::Element> HtmlElement for E {}

macro_rules! simples {
    ($(($t:ident,$id:ident)),*) => {
        $(
            pub struct $t;

            impl write_scope::xml::SimpleElement for $t {
                const TAG: &str = stringify!($id);
            }
        )*
    };
}

simples! {
    (Html, html),
    (Head,head),
    (Body, body),
    (Main, main),
    (Header, header),
    (Div, div),
    (P, p),
    (H1, h1),
    (H2, h2),
    (Ul, ul),
    (Ol, ol),
    (Li, li)
}

pub struct A<S> {
    pub href: S,
}

impl<S: Display> Element for A<S> {
    const TAG: &str = "a";

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        write!(w, "a href={}", self.href)
    }
}

pub struct Img<S> {
    pub src: S,
}

impl<S: AsRef<str>> Element for Img<S> {
    const TAG: &str = "img";

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        write!(w, "img src={}", self.src.as_ref())
    }
}

pub struct Attr<K, V, E> {
    key: K,
    val: V,
    inner: E,
}

impl<K: AsRef<str>, V: Display, E: Element> Element for Attr<K, V, E> {
    const TAG: &str = E::TAG;

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        self.inner.tag_and_attributes(&mut w)?;
        write!(w, " {}=\"{}\"", self.key.as_ref(), self.val)
    }
}

pub struct Id<V, E> {
    val: V,
    inner: E,
}

impl<V: AsRef<str>, E: Element> Element for Id<V, E> {
    const TAG: &str = E::TAG;

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        self.inner.tag_and_attributes(&mut w)?;
        write!(w, " id=\"{}\"", self.val.as_ref())
    }
}

pub struct Class<V, E> {
    val: V,
    inner: E,
}

impl<V: AsRef<str>, E: Element> Element for Class<V, E> {
    const TAG: &str = E::TAG;

    fn tag_and_attributes<W: Open>(self, mut w: W) -> Result<(), W::Error> {
        self.inner.tag_and_attributes(&mut w)?;
        write!(w, " class=\"{}\"", self.val.as_ref())
    }
}
