use binrw::{BinResult, BinWrite};

#[binrw::writer]
fn wrong(_: &u8) -> BinResult<()> {
    Ok(())
}

#[binrw::writer]
fn wrong_args(_: &i32, _: ()) -> BinResult<()> {
    Ok(())
}

#[derive(BinWrite)]
struct Foo {
    #[bw(write_with = 56)]
    a: i32,
    #[bw(write_with = |_: &u8, _, _, _| { Ok(()) })]
    b: i32,
    #[bw(write_with = wrong)]
    c: i32,
    #[bw(write_with = missing)]
    d: i32,
    #[bw(write_with = wrong_args, args(0))]
    e: u8,
}

fn main() {}
