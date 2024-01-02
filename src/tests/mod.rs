/*
 *     MSP430 emulator
 *     Copyright (C) 2023  Sam Wagenaar
 *
 *     This program is free software: you can redistribute it and/or modify
 *     it under the terms of the GNU General Public License as published by
 *     the Free Software Foundation, either version 3 of the License, or
 *     (at your option) any later version.
 *
 *     This program is distributed in the hope that it will be useful,
 *     but WITHOUT ANY WARRANTY; without even the implied warranty of
 *     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *     GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use super::*;
use utils::{assemble, execute, encode_2complement, decode_2complement, wrap_2complement, execute_nr_nd};

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
    execute(c, &trimmed, 14); // need to increase this value when an instruction is added
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
setn ; absolute mode uses sr (r2) as a special case, fuzz it
mov #0xf00d &0xc0de
setc
mov &0xc0de r5
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 4);

    assert_eq!(0xf00d, c.memory.get_word(0xc0de), "Absolute as target");
    assert_eq!(0xf00d, c.get_register(5).get_word(), "Absolute as source");
}

#[test]
fn symbolic_arg_mode() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xf00d 0xc0de
setc
mov 0xc0de r5
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 4);
    
    for offset in -4isize..=4 {
        println!("[0xc0de {}]: {}", offset, c.memory.get_word((0xc0de+offset) as u16));
    }
    println!("{} at 0xc0de+2", c.memory.get_word(0xc0de-1));

    assert_eq!(0xf00d, c.memory.get_word(0xc0de), "Symbolic as target");
    assert_eq!(0xf00d, c.get_register(5).get_word(), "Symbolic as source");
}

#[test]
fn byte_mode() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0xc0de r5
mov.b r5 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 4);

    assert_eq!(0x00de, c.get_register(6).get_word(), "Byte mode");
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
    execute(c, &trimmed, 11);
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
    execute(c, &trimmed, 9);

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
    execute(c, &trimmed, 3);

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
    execute(c, &trimmed, 3);

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
    execute(c, &trimmed, 3);

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
    execute(c, &trimmed, 3);

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
    execute(c, &trimmed, 9);

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
    execute(c, &trimmed, 2);

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
    execute(c, &trimmed, 4);

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
    execute(c, &trimmed, 10);

    assert_eq!(0xc0de, c.get_register(5).get_word(), "Target gets called");
    assert_eq!(0xf00d, c.get_register(6).get_word(), "Return operates properly");
}

#[test]
fn interrupts() {
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
mov #0x4400 sp
mov #2 r5 ; runs to here initially (2 steps), then interrupts
inc r5; should continue here after interrupt

handler:
mov #6 r8
reti

; bind interrupt
.interrupt 0xffa0 handler
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 2);
    assert_eq!(2, c.get_register(5).get_word(), "Pre-interrupt code operates properly");
    // call interrupt
    c.interrupt(0xffa0);
    // execute mov and reti inside of interrupt
    c.step();
    c.step();
    assert_eq!(6, c.get_register(8).get_word(), "Interrupt operates properly");
    // execute post-interrupt instruction
    c.step();
    assert_eq!(3, c.get_register(5).get_word(), "Post-interrupt code operates properly");
}

#[test]
fn jc_jhs() { // jump if carry is set
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
clrc
jc b ; skip a if carry set (it isn't set, so nothing happens here)
a:
mov #0x1 r5
b:
mov #0x1 r6

setc
jc d ; skip c if carry set (it is)
c:
mov #0x1 r7
d:
mov #0x1 r8
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10);

    assert_eq!(1, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
    
    assert_eq!(0, c.get_register(7).get_word(), "r7");
    assert_eq!(1, c.get_register(8).get_word(), "r8");
}

#[test]
fn jeq_jz() { // jump if zero is set
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
clrz
jeq b ; skip a if zero set (it isn't set, so nothing happens here)
a:
mov #0x1 r5
b:
mov #0x1 r6

setz
jeq d ; skip c if zero set (it is)
c:
mov #0x1 r7
d:
mov #0x1 r8
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10);

    assert_eq!(1, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
    
    assert_eq!(0, c.get_register(7).get_word(), "r7");
    assert_eq!(1, c.get_register(8).get_word(), "r8");
}

#[test]
fn jge() { // jump if !(n ^ v)
    /*
    N|V|Jumps|
    0|0|True |
    0|1|False|
    1|0|False|
    1|1|True |
    */
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble(r#"
.define "bic #256,sr", clrv
.define "bis #256,sr", setv
clrn
[clrv]
jge b ; skip a? True
a:
mov #0x1 r5
b:
mov #0x1 r6

clrn
[setv]
jge d ; skip c? False
c:
mov #0x1 r7
d:
mov #0x1 r8

setn
[clrv]
jge f ; skip e? False
e:
mov #0x1 r9
f:
mov #0x1 r10

setn
[setv]
jge h ; skip g? True
g:
mov #0x1 r11
h:
mov #0x1 r12
"#);
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 20);

    assert_eq!(0, c.get_register(5).get_word(), "r5 a");
    assert_eq!(1, c.get_register(6).get_word(), "r6 b");
    
    assert_eq!(1, c.get_register(7).get_word(), "r7 c");
    assert_eq!(1, c.get_register(8).get_word(), "r8 d");
    
    assert_eq!(1, c.get_register(9).get_word(), "r9 e");
    assert_eq!(1, c.get_register(10).get_word(), "r10 f");
    
    assert_eq!(0, c.get_register(11).get_word(), "r11 g");
    assert_eq!(1, c.get_register(12).get_word(), "r12 h");
}

#[test]
fn jl() { // jump if (n ^ v)
    /*
    N|V|Jumps|
    0|0|False|
    0|1|True |
    1|0|True |
    1|1|False|
    */
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble(r#"
.define "bic #256,sr", clrv
.define "bis #256,sr", setv
clrn
[clrv]
jl b ; skip a? False
a:
mov #0x1 r5
b:
mov #0x1 r6

clrn
[setv]
jl d ; skip c? True
c:
mov #0x1 r7
d:
mov #0x1 r8

setn
[clrv]
jl f ; skip e? True
e:
mov #0x1 r9
f:
mov #0x1 r10

setn
[setv]
jl h ; skip g? False
g:
mov #0x1 r11
h:
mov #0x1 r12
"#);
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 20);

    assert_eq!(1, c.get_register(5).get_word(), "r5 a");
    assert_eq!(1, c.get_register(6).get_word(), "r6 b");
    
    assert_eq!(0, c.get_register(7).get_word(), "r7 c");
    assert_eq!(1, c.get_register(8).get_word(), "r8 d");
    
    assert_eq!(0, c.get_register(9).get_word(), "r9 e");
    assert_eq!(1, c.get_register(10).get_word(), "r10 f");
    
    assert_eq!(1, c.get_register(11).get_word(), "r11 g");
    assert_eq!(1, c.get_register(12).get_word(), "r12 h");
}

#[test]
fn jmp() { // unconditional jump
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
jmp b ; skip a
a:
mov #0x1 r5
b:
mov #0x1 r6
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 5);

    assert_eq!(0, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
}

#[test]
fn jn() { // jump if negative
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
clrn
jn b ; don't skip a
a:
mov #0x1 r5
b:
mov #0x1 r6

setn
jn d ; skip c
c:
mov #0x1 r7
d:
mov #0x1 r8
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10);

    assert_eq!(1, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
    
    assert_eq!(0, c.get_register(7).get_word(), "r7");
    assert_eq!(1, c.get_register(8).get_word(), "r8");
}

#[test]
fn jnc_jlo() { // jump if !carry
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
clrc
jnc b ; skip a
a:
mov #0x1 r5
b:
mov #0x1 r6

setc
jnc d ; dont' skip c
c:
mov #0x1 r7
d:
mov #0x1 r8
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10);

    assert_eq!(0, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
    
    assert_eq!(1, c.get_register(7).get_word(), "r7");
    assert_eq!(1, c.get_register(8).get_word(), "r8");
}



#[test]
fn jne_jnz() { // jump if !zero
    let c: &mut Computer = &mut Computer::new();
    let assembled = assemble("
clrz
jnz b ; skip a
a:
mov #0x1 r5
b:
mov #0x1 r6

setz
jnz d ; dont' skip c
c:
mov #0x1 r7
d:
mov #0x1 r8
");
    let trimmed = assembled.trim();
    println!("'{}'", trimmed);
    execute(c, &trimmed, 10);

    assert_eq!(0, c.get_register(5).get_word(), "r5");
    assert_eq!(1, c.get_register(6).get_word(), "r6");
    
    assert_eq!(1, c.get_register(7).get_word(), "r7");
    assert_eq!(1, c.get_register(8).get_word(), "r8");
}

/***********/
/* Fuzzing */
/***********/

// this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
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

// this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
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

// this does 4.2 billion assembly + emulation runs (which takes 7.5 minutes)
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
