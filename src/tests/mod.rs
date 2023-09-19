use super::*;
use base64::{Engine as _, engine::general_purpose};
use std::io::prelude::*;
use std::process::{Command, Stdio};

fn assemble(code: &str) -> String {
    let mut child = Command::new("./tools/assembler")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run assembler");
    child.stdin.take().unwrap().write_all(code.as_bytes()).expect("Failed to write code to assembler");
    let mut buf: String = "".to_string();
    child.stdout.take().unwrap().read_to_string(&mut buf).expect("Failed to receive assembled bytes back");

    if buf.starts_with("<FAILURE>") {
        panic!("  Failed to assemble `{}`  ", code);
    }

    return buf;
}

fn execute(computer: &mut Computer, data: &str, steps: u64) {
    computer.reset();
    execute_nr(computer, data, steps);
}

fn execute_nr(computer: &mut Computer, data: &str, steps: u64) { // no reset
    let byte_data: Vec<u8> = match general_purpose::STANDARD.decode(data) {
        Ok(v) => v,
        Err(_) => panic!("Failed to decode memory")
    };

    let start: u16 = ((byte_data[0] as u16) << 8) + (byte_data[1] as u16);
    computer.pc.set_word(start - 2);
    
    for i in 2..byte_data.len() {
        let idx = (((start as usize) + i - 2) & 0xffff) as u16;
        let val = byte_data[i];
        println!("idx={}, val={}", idx, val);
        computer.memory.set_byte(idx, val);
    }

    for _ in 0..=steps {
        computer.step();
    }
}

#[test]
fn register_truncation() {
    let reg: &mut BasicRegister = &mut BasicRegister::new(4);
    reg.set_word(0xf0a0);
    reg.set_byte(0xa5);

    assert_eq!(0xa5, reg.get_word());
}

#[test]
fn decode_enum() {
    assert_eq!(DoubleOperandOpcodes::try_from(0u8), Ok(DoubleOperandOpcodes::MOV));
}

#[test]
fn mov_and_arg_modes() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xf00d r6
mov #0xf00d r8
mov #0xc0de 0(r6)
mov @r6 r7
mov @r8+ r9
mov #0xface 5(r6)
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 6); // WARN: need to increase this value when an instruction is added
    assert_eq!(0xf00d, c.get_register(6).get_word(), "Basic register");
    assert_eq!(0xc0de, c.memory.get_word(0xf00d), "Indexed");
    assert_eq!(0xc0de, c.get_register(7).get_word(), "Indirect");
    assert_eq!(0xf00d + 2, c.get_register(8).get_word(), "Autoincrement");
    assert_eq!(0xface, c.memory.get_word(0xf00d + 5));
}
