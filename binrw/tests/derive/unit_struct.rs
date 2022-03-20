use binrw::{io::Cursor, BinRead};

// Since proc-macros are unhygienic, make sure they are not generating code that
// may accidentally use the wrong thing
#[allow(non_snake_case)]
fn Ok() {}
#[allow(non_snake_case)]
fn Err() {}

#[test]
fn unit_struct_magic() {
    #[derive(BinRead, Debug)]
    #[br(big, magic = 1u16)]
    struct Test;

    Test::read(&mut Cursor::new(b"\x00\x01")).unwrap();
    let error = Test::read(&mut Cursor::new(b"\x00\x00")).expect_err("accepted bad data");
    assert!(matches!(error, binrw::Error::BadMagic { .. }));
}

#[test]
fn unit_struct_import_pre_assert() {
    #[derive(BinRead, Debug)]
    #[br(import(succeed: bool), pre_assert(succeed))]
    struct Test;

    Test::read_args(&mut Cursor::new(b""), (true,)).unwrap();
    let error =
        Test::read_args(&mut Cursor::new(b""), (false,)).expect_err("accepted negative pre-assert");
    assert!(matches!(error, binrw::Error::AssertFail { .. }));
}
