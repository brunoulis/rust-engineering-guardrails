//! Integration test: skipped unless --include-tests.
#[test]
fn it() {
    helper().unwrap();
}
fn helper() -> Option<u8> {
    None::<u8>.unwrap();
    None
}
