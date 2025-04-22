use std::io::{self, stdout};

use write_html::{Body, Div, Html, HtmlElement, H1, P};
use write_scope::{Open, WrapIO};

fn main() -> io::Result<()> {
    let w = WrapIO(stdout().lock());
    let html = w.open(Html)?;
    let mut body = html.open(Body)?;
    {
        let id = String::from("toto");
        let mut toto = body.ref_open(Div.id(&id).id("babar"))?.open(Div)?;
        drop(id);
        toto.ref_open(Div.id("toto_1"))?;
        toto.ref_open(Div.id("toto_2"))?;
        toto.close()?;
    }
    body.ref_open(Div.id("tata"))?;
    body.open_scope(Div.id("tutu"), |tutu| {
        tutu.open(H1)?.text("Hello!")?;
        tutu.open(P)?.text("Beautiful morning!")?;
        Ok(())
    })?;
    body.open(Div)?;
    Ok(())
}
