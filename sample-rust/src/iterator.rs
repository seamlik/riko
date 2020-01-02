#[riko::fun(marshal = "Iterator<String>")]
fn iterator(a: String, b: String) -> Box<dyn Iterator<Item = String> + Send + 'static> {
    Box::new(vec![a, b].into_iter())
}

#[riko::fun(marshal = "Iterator<String>")]
fn iterator_fallible(
    item: String,
    fails: bool,
) -> Result<impl Iterator<Item = String> + Send + 'static, std::fmt::Error> {
    if fails {
        Err(std::fmt::Error)
    } else {
        Ok(std::iter::once(item))
    }
}
