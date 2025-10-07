wit_bindgen::generate!({
    world: "instructions",
});

struct InstructionsWorld;

export!(InstructionsWorld);

impl Guest for InstructionsWorld {
    fn s8_roundtrip(val: i8) -> i8 {
        assert!((i8::MIN..=i8::MAX).contains(&val));
        val
    }
    fn u8_roundtrip(val: u8) -> u8 {
        assert!((u8::MIN..=u8::MAX).contains(&val));
        val
    }
    fn s16_roundtrip(val: i16) -> i16 {
        assert!((i16::MIN..=i16::MAX).contains(&val));
        val
    }
    fn u16_roundtrip(val: u16) -> u16 {
        assert!((u16::MIN..=u16::MAX).contains(&val));
        val
    }
    fn s32_roundtrip(val: i32) -> i32 {
        assert!((i32::MIN..=i32::MAX).contains(&val));
        val
    }
    fn u32_roundtrip(val: u32) -> u32 {
        assert!((u32::MIN..=u32::MAX).contains(&val));
        val
    }
}
