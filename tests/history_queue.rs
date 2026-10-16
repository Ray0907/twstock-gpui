use twstock_gpui::market::*;
fn sym(code: &str) -> Symbol {
    Symbol::new(Market::Listed, code).unwrap()
}
#[test]
fn selected_first_then_watchlist_order_and_selection_preempts() {
    let symbols = vec![sym("AAAA"), sym("BBBB"), sym("CCCC")];
    let mut queue = HistoryQueue::default();
    queue.enqueue(&symbols, Some(&symbols[1]));
    assert_eq!(queue.next(), Some(symbols[1].clone()));
    queue.select(&symbols[2]);
    assert_eq!(queue.next(), Some(symbols[2].clone()));
    // Completion of the old in-flight job must not remove the new front.
    queue.finish(&symbols[1]);
    assert_eq!(queue.next(), Some(symbols[2].clone()));
    queue.finish(&symbols[2]);
    assert_eq!(queue.next(), Some(symbols[0].clone()));
}
#[test]
fn duplicate_loaded_and_new_selected_instruments() {
    let a = sym("AAAA");
    let b = sym("BBBB");
    let mut queue = HistoryQueue::default();
    queue.enqueue(&[a.clone(), a.clone()], Some(&a));
    queue.finish(&a);
    queue.enqueue(std::slice::from_ref(&a), Some(&a));
    queue.select(&a);
    assert_eq!(queue.next(), None);
    queue.enqueue(&[a.clone(), b.clone()], Some(&b));
    assert_eq!(queue.next(), Some(b.clone()));
    queue.finish(&b);
    assert_eq!(queue.next(), None);
    queue.retain(std::slice::from_ref(&b));
    // A removed in-flight request must not mark a later re-add as already loaded.
    queue.finish(&a);
    queue.enqueue(&[a.clone(), b], Some(&a));
    assert_eq!(queue.next(), Some(a));
}
