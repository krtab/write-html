use std::io::{self, stdout};

use write_html::{Body, Div, Html, HtmlElement, H1, P};
use write_scope::{Open, WrapIO};



fn main() -> io::Result<()> {
    let mut w = WrapIO(stdout().lock());
    let mut html = w.ref_open(Html)?;
    let mut body = html.ref_open(Body)?;
    let id = String::from("toto");
    let mut toto = body.ref_open(Div.id(&id).id("babar"))?.open(Div)?;
    drop(id);
    toto.ref_open(Div.id("toto_1"))?;
    toto.ref_open(Div.id("toto_2"))?;
    toto.close()?;
    body.ref_open(Div.id("tata"))?;
    body.open_scope(Div.id("tutu"), |tutu| {
        tutu.ref_open(H1)?.text("Hello!")?;
        tutu.ref_open(P)?.text("Beautiful morning!")?;
        Ok(())
    })?;
    body.ref_open(Div)?;
    Ok(())
}
