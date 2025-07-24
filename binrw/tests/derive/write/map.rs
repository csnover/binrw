extern crate binrw;
use super::t;

#[test]
fn map_field() {
    #[binrw::binwrite]
    #[bw(big)]
    struct Test {
        #[bw(map = |&x| x as u64)]
        x: u32,

        #[bw(map = |x| x.as_bytes())]
        y: t::String,

        #[bw(calc = 0xff, map = |x: u8| x)]
        z: u8,
    }

    let mut x = binrw::io::Cursor::new(t::Vec::new());

    binrw::BinWrite::write(
        &Test {
            x: 3,
            y: <t::String as t::From<_>>::from("test"),
        },
        &mut x,
    )
    .unwrap();

    t::assert_eq!(x.into_inner(), b"\0\0\0\0\0\0\0\x03test\xff");
}

#[test]
fn map_field_code_coverage() {
    #[allow(dead_code)]
    #[derive(binrw::BinWrite)]
    struct Test {
        #[bw(map = |&x| x as u64)]
        x: u32,

        #[bw(map = |x| x.as_bytes())]
        y: t::String,
    }
}

#[test]
fn map_repr_enum() {
    #[allow(dead_code)]
    #[derive(binrw::BinWrite, Debug)]
    #[bw(repr = u8)]
    enum Test {
        SubTest(u8),
    }

    impl t::From<&Test> for u8 {
        fn from(t: &Test) -> Self {
            match t {
                Test::SubTest(u) => *u,
            }
        }
    }
}

#[test]
fn map_repr_enum_variant() {
    #[allow(dead_code)]
    #[derive(binrw::BinWrite, Debug)]
    enum Test {
        SubTest(#[bw(repr = u8)] SubTest),
    }

    #[derive(Debug)]
    struct SubTest(u8);

    impl t::From<&SubTest> for u8 {
        fn from(s: &SubTest) -> Self {
            s.0
        }
    }
}

#[test]
fn map_repr_struct() {
    #[derive(binrw::BinWrite, Debug)]
    #[bw(repr = u8)]
    struct Test {
        a: u8,
    }

    impl t::From<&Test> for u8 {
        fn from(t: &Test) -> Self {
            t.a
        }
    }
}

#[test]
fn map_repr_struct_field() {
    #[allow(dead_code)]
    #[derive(binrw::BinWrite, Debug)]
    #[bw(big)]
    struct Test {
        #[bw(repr = u8)]
        a: SubTest,
    }

    #[derive(Debug)]
    struct SubTest {
        a: u8,
    }

    impl t::From<&SubTest> for u8 {
        fn from(s: &SubTest) -> Self {
            s.a
        }
    }
}

#[test]
fn try_map() {
    #[derive(binrw::BinWrite)]
    struct MyType {
        #[bw(try_map = |&x| { <i8 as t::TryFrom<_>>::try_from(x) })]
        value: u8,
    }

    let mut x = binrw::io::Cursor::new(t::Vec::new());
    binrw::BinWrite::write_le(&MyType { value: 127 }, &mut x).unwrap();
    t::assert_eq!(x.into_inner(), b"\x7f");

    let mut x = binrw::io::Cursor::new(t::Vec::new());
    binrw::BinWrite::write_le(&MyType { value: 128 }, &mut x).unwrap_err();
}

#[test]
fn map_write_with() {
    #[derive(binrw::BinWrite)]
    struct MyType {
        #[bw(map = |&x| x as u16, write_with = <u16 as binrw::BinWrite>::write_options)]
        value: u8,
    }

    let mut x = binrw::io::Cursor::new(t::Vec::new());
    binrw::BinWrite::write_le(&MyType { value: 127 }, &mut x).unwrap();
    t::assert_eq!(x.into_inner(), b"\x7f\0");
}

#[test]
fn map_lifetime_args() {
    #[derive(binrw::BinWrite)]
    #[bw(import(borrowed: &u8))]
    struct Wrapper(#[bw(map = |&x| x + *borrowed)] u8);

    #[derive(binrw::BinWrite, Debug, PartialEq)]
    #[bw(little, import(borrowed: &u8))]
    struct Test {
        #[bw(map = |&x| Wrapper(x), args(borrowed))]
        a: u8,
    }

    let mut x = binrw::io::Cursor::new(t::Vec::new());
    binrw::BinWrite::write_args(&Test { a: 1 }, &mut x, (&1_u8,)).unwrap();

    t::assert_eq!(x.into_inner(), b"\x02");
}

#[test]
fn try_map_lifetime_args() {
    #[derive(binrw::BinWrite)]
    #[bw(import(borrowed: &u8))]
    struct Wrapper(#[bw(map = |&x| x + *borrowed)] u8);

    fn try_map_wrapper(x: &u8) -> binrw::BinResult<Wrapper> {
        t::Ok(Wrapper(*x))
    }

    #[derive(binrw::BinWrite, Debug, PartialEq)]
    #[bw(little, import(borrowed: &u8))]
    struct Test {
        #[bw(try_map = try_map_wrapper, args(borrowed))]
        a: u8,
    }

    let mut x = binrw::io::Cursor::new(t::Vec::new());
    binrw::BinWrite::write_args(&Test { a: 1 }, &mut x, (&1_u8,)).unwrap();

    t::assert_eq!(x.into_inner(), b"\x02");
}
