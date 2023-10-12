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
use base64::{Engine as _, engine::general_purpose};
use std::io::Write;
use std::process::{Command, Stdio};

#[allow(dead_code)]
pub(crate) fn decode_2complement(v: u16) -> i32 {
    if v > 0x7fff {
        return (v as i32) - 0x10000;
    } else {
        return v as i32;
    }
}

#[allow(dead_code)]
pub(crate) fn encode_2complement(v: i32) -> u16 {
    if v < 0 {
        return ((v + 0x10000) & 0xffff) as u16;
    } else {
        return (v & 0xffff) as u16;
    }
}

#[allow(dead_code)]
pub(crate) fn wrap_2complement(v: i32) -> i32 {
    return decode_2complement(encode_2complement(v));
}

#[allow(dead_code)]
pub(crate) fn assemble(code: &str) -> String {
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

#[allow(dead_code)]
pub(crate) fn execute(computer: &mut Computer, data: &str, steps: u64) {
    computer.reset();
    execute_nr(computer, data, steps);
}

#[allow(dead_code)]
pub(crate) fn execute_nd(computer: &mut Computer, byte_data: &[u8], steps: u64) {
    computer.reset();
    execute_nr_nd(computer, byte_data, steps);
}

#[allow(dead_code)]
pub(crate) fn execute_nr(computer: &mut Computer, data: &str, steps: u64) {
    let byte_data: Vec<u8> = match general_purpose::STANDARD.decode(data) {
        Ok(v) => v,
        Err(_) => panic!("Failed to decode memory")
    };
    execute_nr_nd(computer, &byte_data, steps);
}

#[allow(dead_code)]
pub(crate) fn execute_nr_nd(computer: &mut Computer, byte_data: &[u8], steps: u64) { // no reset
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
