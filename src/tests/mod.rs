use super::*;
use base64::{Engine as _, engine::general_purpose};
use std::io::prelude::*;
use std::process::{Command, Stdio};

fn decode_2complement(v: u16) -> i32 {
    if v > 0x7fff {
        return (v as i32) - 0x10000;
    } else {
        return v as i32;
    }
}

fn encode_2complement(v: i32) -> u16 {
    if v < 0 {
        return ((v + 0x10000) & 0xffff) as u16;
    } else {
        return (v & 0xffff) as u16;
    }
}

fn wrap_2complement(v: i32) -> i32 {
    return decode_2complement(encode_2complement(v));
}

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

#[allow(dead_code)]
fn execute_nd(computer: &mut Computer, byte_data: &[u8], steps: u64) {
    computer.reset();
    execute_nr_nd(computer, byte_data, steps);
}

fn execute_nr(computer: &mut Computer, data: &str, steps: u64) {
    let byte_data: Vec<u8> = match general_purpose::STANDARD.decode(data) {
        Ok(v) => v,
        Err(_) => panic!("Failed to decode memory")
    };
    execute_nr_nd(computer, &byte_data, steps);
}

fn execute_nr_nd(computer: &mut Computer, byte_data: &[u8], steps: u64) { // no reset
    /*let byte_data: Vec<u8> = match general_purpose::STANDARD.decode(data) {
        Ok(v) => v,
        Err(_) => panic!("Failed to decode memory")
    };*/

    let start: u16 = ((byte_data[0] as u16) << 8) + (byte_data[1] as u16);
    computer.pc.set_word(start - 2);
    
    for i in 2..byte_data.len() {
        let idx = (((start as usize) + i - 2) & 0xffff) as u16;
        let val = byte_data[i];
        //println!("idx={}, val={}", idx, val);
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

; test 'compressed' literals
; free registers: 10, 11, 12, 13, 14, 15
; needed numbers: -1,  0,  1,  2,  4,  8
mov #-1 r10
mov #0 r11
mov #1 r12
mov #2 r13
mov #4 r14
mov #8 r15
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 14); // WARN: need to increase this value when an instruction is added
    assert_eq!(0xf00d, c.get_register(6).get_word(), "Basic register");
    assert_eq!(0xc0de, c.memory.get_word(0xf00d), "Indexed");
    assert_eq!(0xc0de, c.get_register(7).get_word(), "Indirect");
    assert_eq!(0xf00d + 2, c.get_register(8).get_word(), "Autoincrement");
    assert_eq!(0xface, c.memory.get_word(0xf00d + 5));

    assert_eq!(0xffff /* -1 */, c.get_register(10).get_word(), "Literal -1");
    assert_eq!(0, c.get_register(11).get_word(), "Literal 0");
    assert_eq!(1, c.get_register(12).get_word(), "Literal 1");
    assert_eq!(2, c.get_register(13).get_word(), "Literal 2");
    assert_eq!(4, c.get_register(14).get_word(), "Literal 4");
    assert_eq!(8, c.get_register(15).get_word(), "Literal 8");
}

#[test]
fn absolute_arg_mode() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xf00d &0xc0de
mov &0xc0de r5
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 2); // WARN: See above

    assert_eq!(0xf00d, c.memory.get_word(0xc0de), "Absolute as target");
    assert_eq!(0xf00d, c.get_register(5).get_word(), "Absolute as source");
}

#[test]
fn add_and_carry() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0x4400 sp
; basic addition
mov #1 r5
mov #2 r6
add r5 r6
addc #0 r9

; overflowing
mov #0xffff r7
mov #1 r8
add r7 r8
push sr ; ensure that flags can be analyzed after execution
addc #0 r10
pop sr  ; restore flags
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 11); // WARN: See above
    assert_eq!(3, c.get_register(6).get_word(), "Basic addition");
    assert_eq!(0, c.get_register(9).get_word(), "Carry: Antiexample");
    
    assert_eq!(0, c.get_register(8).get_word(), "Overflowing");
    assert_eq!(1, c.get_register(10).get_word(), "Carry");

    assert_eq!(false, c.sr.get_status(StatusFlags::NEGATIVE), "Flag: N");
    assert_eq!(true,  c.sr.get_status(StatusFlags::ZERO),     "Flag: Z");
    assert_eq!(true,  c.sr.get_status(StatusFlags::CARRY),    "Flag: C");
    assert_eq!(false, c.sr.get_status(StatusFlags::OVERFLOW), "Flag: V");
}

#[test]
fn sub_manual() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
; basic subtraction
mov #1 r5
mov #3 r6
sub r5 r6

; overflowing
mov #0xffff r7
mov #1 r8
sub r7 r8

; overflowing (the other way)
mov #1 r9
mov #0xffff r10
sub r9 r10
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 9); // WARN: See above

    assert_eq!(2, c.get_register(6).get_word());
    assert_eq!(2, c.get_register(8).get_word());
    assert_eq!(0xfffe, c.get_register(10).get_word()); // -2 in two's complement
}

#[test]
fn bic() { // BIt Clear
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xff00 r5
mov #0x0ff0 r6
bic r5 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 3); // WARN: See above

    assert_eq!(0x00f0, c.get_register(6).get_word());
}

#[test]
fn bis() { // BIt Set
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xff00 r5
mov #0x0ff0 r6
bis r5 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 3); // WARN: See above

    assert_eq!(0xfff0, c.get_register(6).get_word());
}

#[test]
fn xor() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xff00 r5
mov #0x0ff0 r6
xor r5 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 3); // WARN: See above

    assert_eq!(0xf0f0, c.get_register(6).get_word());
}

#[test]
fn and() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xff00 r5
mov #0x0ff0 r6
and r5 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 3); // WARN: See above

    assert_eq!(0x0f00, c.get_register(6).get_word());
}

#[test]
fn rrc_rra() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #16 r6
rrc r6

push sr

mov #15 r7
rrc r7
rrc r8

mov #-4 r9
rra r9

pop sr
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 9); // WARN: See above

    assert_eq!(8, c.get_register(6).get_word(), "RRC (part 1)");
    assert_eq!(false, c.sr.get_status(StatusFlags::CARRY), "Flags: C");
    assert_eq!(0b1000_0000_0000_0000, c.get_register(8).get_word(), "RRC (part 2)");
    assert_eq!(encode_2complement(-2), c.get_register(9).get_word(), "RRA -4 / 2 = -2");
}

#[test]
fn swpb() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xff00 r6
swpb r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 2); // WARN: See above

    assert_eq!(0x00ff, c.get_register(6).get_word());
}

#[test]
fn sxt() { // sign extend
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0x0e r5
mov #0xfe r6
sxt r5
sxt r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 4); // WARN: See above

    assert_eq!(0x000e, c.get_register(5).get_word());
    assert_eq!(0xfffe, c.get_register(6).get_word());
}

#[test]
fn call() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0x4400 sp
call #target
mov #0xf00d r6
jmp 0
mov #0x1 r5 ; should never reach here

target:
mov #0xc0de r5
ret

mov #0x0 r5 ; should never reach here
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10); // WARN: See above

    assert_eq!(0xc0de, c.get_register(5).get_word(), "Target gets called");
    assert_eq!(0xf00d, c.get_register(6).get_word(), "Return operates properly");
}

/***********/
/* Fuzzing */
/***********/

// WARN: this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
#[test]
#[ignore]
fn sub_fuzz() {
    let c: &mut Computer = &mut Computer::new();
    for first in 0..=0xffffu16 {
        //println!("Starting series: first = {}", first);
        for second in 0..=0xffffu16 {
            let f_i: i32 = decode_2complement(first);
            let s_i: i32 = decode_2complement(second);

            /*let assembled = assemble(&format!("
mov #{0} r5
mov #{1} r6
sub r5 r6
", f_i, s_i));
            let trimmed0 = assembled.trim();
            println!("t0: '{}'", trimmed0);*/
            let f_lower: u8 = (first & 0xff) as u8;
            let f_upper: u8 = ((first >> 8) & 0xff) as u8;
            
            let s_lower: u8 = (second & 0xff) as u8;
            let s_upper: u8 = ((second >> 8) & 0xff) as u8;
            
            let data: [u8; 12] = [
                0x44, 0x00,                   // start-of-code header
                0x40, 0x35, f_upper, f_lower, // mov #{first} r5
                0x40, 0x36, s_upper, s_lower, // mov #{second} r6
                0x85, 0x06];                  // sub r5 r6
            //println!("first: {}i32 ({}u16), second: {}i32 ({}u16)", f_i, first, s_i, second);
            //println!("'{}'", trimmed);
            execute_nr_nd(c, &data, 3);
            let expected_result = wrap_2complement(s_i - f_i);
            //println!("Expected: {}, actual: {}u16 ({}i32)", expected_result, c.get_register(6).get_word(), decode_2complement(c.get_register(6).get_word()));
            assert_eq!(expected_result, decode_2complement(c.get_register(6).get_word()), "Fuzzing");
        }
    }
}

// WARN: this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
#[test]
#[ignore]
fn subc_off_fuzz() {
    let c: &mut Computer = &mut Computer::new();
    for first in 0..=0xffffu16 {
        for second in 0..=0xffffu16 {
            let f_i: i32 = decode_2complement(first);
            let s_i: i32 = decode_2complement(second);

            let f_lower: u8 = (first & 0xff) as u8;
            let f_upper: u8 = ((first >> 8) & 0xff) as u8;
            
            let s_lower: u8 = (second & 0xff) as u8;
            let s_upper: u8 = ((second >> 8) & 0xff) as u8;
            
            let data: [u8; 12] = [
                0x44, 0x00,                   // start-of-code header
                0x40, 0x35, f_upper, f_lower, // mov #{first} r5
                0x40, 0x36, s_upper, s_lower, // mov #{second} r6
                0x75, 0x06];                  // subc r5 r6
            c.sr.set_status(StatusFlags::CARRY, false);
            execute_nr_nd(c, &data, 3);
            let expected_result = wrap_2complement(s_i - f_i - 1);
            assert_eq!(expected_result, decode_2complement(c.get_register(6).get_word()), "Fuzzing");
        }
    }
}

// WARN: this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
#[test]
#[ignore]
fn subc_on_fuzz() {
    let c: &mut Computer = &mut Computer::new();
    for first in 0..=0xffffu16 {
        for second in 0..=0xffffu16 {
            let f_i: i32 = decode_2complement(first);
            let s_i: i32 = decode_2complement(second);

            let f_lower: u8 = (first & 0xff) as u8;
            let f_upper: u8 = ((first >> 8) & 0xff) as u8;
            
            let s_lower: u8 = (second & 0xff) as u8;
            let s_upper: u8 = ((second >> 8) & 0xff) as u8;
            
            let data: [u8; 12] = [
                0x44, 0x00,                   // start-of-code header
                0x40, 0x35, f_upper, f_lower, // mov #{first} r5
                0x40, 0x36, s_upper, s_lower, // mov #{second} r6
                0x75, 0x06];                  // subc r5 r6
            c.sr.set_status(StatusFlags::CARRY, true);
            execute_nr_nd(c, &data, 3);
            let expected_result = wrap_2complement(s_i - f_i);
            assert_eq!(expected_result, decode_2complement(c.get_register(6).get_word()), "Fuzzing");
        }
    }
}

#[allow(dead_code)]
fn t() {
    // TODO: more testing for subc, there might still be some panics
    
    // TODO: for sub/subc, have a normal subtraction and an overflowing subtraction (may require signed
    // magic)
}
