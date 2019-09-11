use riko_runtime::iterators::MarshalingIterator;

fn iterator() -> impl Iterator<Item = String> {
    let list = vec!["I".to_owned(), "love".to_owned(), "you".to_owned()];
    list.into_iter()
}

#[riko::fun(rename = "iterator", sig = "() -> Iterator<String>")]
fn iterator_ffi() -> MarshalingIterator {
    MarshalingIterator::new(iterator())
}
