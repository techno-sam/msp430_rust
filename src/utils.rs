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
    load_code(computer, byte_data);
    for _ in 0..steps {
        computer.step();
    }
}

pub(crate) fn load_code(computer: &mut Computer, byte_data: &[u8]) {
    let new_fmt_data: &[u8];
    let tmp; // lifetime issues
    if byte_data[0] != 0xff || byte_data[1] != 0xff { // if magic marker is not detected, convert
        tmp = convert_code_fmt(byte_data);
        new_fmt_data = &tmp;
    } else {
        new_fmt_data = byte_data;
    }
    load_code_fmt_new(computer, new_fmt_data);
}

/* // kept for reference
pub(crate) fn load_code_fmt_old(computer: &mut Computer, byte_data: &[u8]) {
    let start: u16 = ((byte_data[0] as u16) << 8) + (byte_data[1] as u16);
    computer.pc.set_word(start - 2);

    for i in 2..byte_data.len() {
        let idx = (((start as usize) + i - 2) & 0xffff) as u16;
        let val = byte_data[i];
        computer.memory.set_byte(idx, val);
    }
}*/

pub(crate) struct U8Stream<'a> {
    _data: &'a[u8],
    _index: usize,
}

impl <'a>U8Stream<'a> {
    pub(crate) fn new(data: &'a[u8]) -> U8Stream<'a> {
        return U8Stream { _data: data, _index: 0 };
    }

    pub(crate) fn pop_byte(self: &mut U8Stream<'a>) -> u8 {
        let out = self._data[self._index];
        self._index += 1;
        return out;
    }

    pub(crate) fn pop_word(self: &mut U8Stream<'a>) -> u16 {
        return ((self.pop_byte() as u16) << 8) + (self.pop_byte() as u16);
    }
}

pub(crate) fn convert_code_fmt(byte_data: &[u8]) -> Vec<u8> {
    let mut converted: Vec<u8> = Vec::new();

    // write marker
    converted.push(0xff);
    converted.push(0xff);

    // write segment count 0x0002 (2 segments, 1 is code, the other is startup vector)
    converted.push(0x00);
    converted.push(0x02);

    /* write code */
    // write start address
    converted.push(byte_data[0]);
    converted.push(byte_data[1]);
    let code_size: u16 = ((byte_data.len() - 2) & 0xffff) as u16;
    // write code size
    converted.push(((code_size & 0xff00) >> 8) as u8);
    converted.push((code_size & 0x00ff) as u8);
    // write code
    for i in 2..byte_data.len() {
        converted.push(byte_data[i]);
    }

    /* write vector table */
    // write start address (0xfffe)
    converted.push(0xff);
    converted.push(0xfe);
    // write segment length (0x0002)
    converted.push(0x00);
    converted.push(0x02);
    // write start address
    converted.push(byte_data[0]);
    converted.push(byte_data[1]);
    return converted;
}

/// This does NOT reset the computer, other than loading the PC
pub(crate) fn load_code_fmt_new(computer: &mut Computer, byte_data: &[u8]) {
    let mut d = U8Stream::new(byte_data);
    if d.pop_word() != 0xffff {
        panic!("Invalid marker for new format");
    }

    let segment_count: u16 = d.pop_word();
    for _ in 0..segment_count {
        let start_addr: u16 = d.pop_word();
        let segment_length: u16 = d.pop_word();
        for offset in 0..segment_length {
            computer.memory.set_byte(start_addr + offset, d.pop_byte());
        }
    }
    computer.pc.set_word(computer.memory.get_word(0xfffe));
}
