fn braille_bytes(codepoint: u32) {
    let x = std::char::from_u32(codepoint).unwrap().to_string();
    let [a, b, c]: [u8; 3] = x.as_bytes().try_into().unwrap();

    println!("{x} 0x{codepoint:0X} {a:0b} {b:0b} {c:0b} {a} {b} {c}");
}

fn main() {
    for x in 0x2800..=0x28FF {
        braille_bytes(x);
    }
}
