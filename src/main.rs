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

use std::{time::Instant, fs::File, io::Read, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use libc::c_char;
use std::ffi::CStr;
use std::str;

use bitflags::bitflags;
use num_enum::TryFromPrimitive;
use clap::Parser;
use shared_memory::{ShmemConf, ShmemError};
use fork::{daemon, Fork};

#[derive(Parser)]
#[clap(author, version, about)]
enum CLI {
    Benchmark,
    Run,
    RunForked
}

trait RegisterData {
    fn get_word(&self) -> u16;
    fn get_byte(&self) -> u8;
    fn set_word(&mut self, value: u16);
    fn set_byte(&mut self, value: u8);
    fn get_id(&self) -> u8;
}


#[derive(Copy, Clone)]
struct BasicRegister {
    id: u8,
    _value: u16
}
impl BasicRegister {
    fn new(id: u8) -> BasicRegister {
        return BasicRegister {
            id,
            _value: 0
        };
    }
}

struct EvenRegister {
    id: u8,
    _value: u16
}
impl EvenRegister {
    fn new(id: u8) -> EvenRegister {
        return EvenRegister {
            id,
            _value: 0
        };
    }
}

struct StatusRegister {
    _value: u16
}


impl RegisterData for BasicRegister {
    fn get_word(&self) -> u16 {
        return self._value;
    }

    fn get_byte(&self) -> u8 {
        return (self._value & 0xff).try_into().unwrap();
    }

    fn set_word(&mut self, value: u16) {
        self._value = value;
    }

    fn set_byte(&mut self, value: u8) {
        self._value = (value & 0xff).try_into().unwrap();
    }
    
    fn get_id(&self) -> u8 {
        return self.id;
    }
}

impl RegisterData for EvenRegister {
    fn get_word(&self) -> u16 {
        return self._value;
    }

    fn get_byte(&self) -> u8 {
        return (self._value & 0xff).try_into().unwrap();
    }

    fn set_word(&mut self, value: u16) {
        self._value = value & 0xfffe;
    }

    fn set_byte(&mut self, value: u8) {
        self._value = (value & 0xfe).try_into().unwrap();
    }

    fn get_id(&self) -> u8 {
        return self.id;
    }
}

impl RegisterData for StatusRegister {
    fn get_word(&self) -> u16 {
        return self._value;
    }

    fn get_byte(&self) -> u8 {
        return (self._value & 0xff).try_into().unwrap();
    }

    fn set_word(&mut self, value: u16) {
        self._value = value;
    }

    fn set_byte(&mut self, value: u8) {
        self._value = (value & 0xff).try_into().unwrap();
    }
    
    fn get_id(&self) -> u8 {
        return 2;
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug,Copy,Clone)]
    pub struct StatusFlags: u16 {
        const CARRY    = 0x001;
        const ZERO     = 0x002;
        const NEGATIVE = 0x004;
        const GIE      = 0x008;
        const CPUOFF   = 0x010;
        const OVERFLOW = 0x100;

        // any bits may be set
        const _ = !0;
    }
}

#[allow(dead_code)]
impl StatusRegister {
    fn new() -> StatusRegister {
        return StatusRegister {_value: 0 };
    }

    fn get_status(&self, flag: StatusFlags) -> bool {
        return self.get_word() & flag.bits() != 0;
    }

    fn set_status(&mut self, flag: StatusFlags, set: bool) {
        if set {
            self.set_word(self.get_word() | flag.bits());
        } else {
            self.set_word(self.get_word() & !flag.bits());
        }
    }
}

struct ConstantGeneratorRegister {}
impl ConstantGeneratorRegister {
    fn new() -> ConstantGeneratorRegister {
        return ConstantGeneratorRegister {};
    }
}

impl RegisterData for ConstantGeneratorRegister {
    fn get_word(&self) -> u16 {
return 0;
    }

    fn get_byte(&self) -> u8 {
        return 0;
    }

    fn set_word(&mut self, _value: u16) {}

    fn set_byte(&mut self, _value: u8) {}
    
    fn get_id(&self) -> u8 {
        return 3;
    }
}

struct MemoryMap {
    _memory: [u8; 0x10000],
}

#[allow(dead_code)]
impl MemoryMap {
    fn new() -> MemoryMap {
        return MemoryMap {
            _memory: [0; 0x10000]
        };
    }

    fn reset(&mut self) {
        self._memory = [0; 0x10000];
    }

    fn get_word(&self, index: u16) -> u16 {
        //assert_eq!(index % 2, 0);
        return ((self._memory[index as usize] as u16) << 8u16) + (self._memory[(index as usize + 1) & 0xffff] as u16);
    }

    fn set_word(&mut self, index: u16, value: u16) {
        //assert_eq!(index % 2, 0);
        self._memory[index as usize] = ((value >> 8) & 0xff) as u8;
        self._memory[(index as usize + 1) & 0xffff] = (value & 0xff) as u8;
    }

    fn get_byte(&self, index: u16) -> u8 {
        return self._memory[index as usize];
    }

    fn set_byte(&mut self, index: u16, value: u8) {
        self._memory[index as usize] = value;
    }
}

trait WriteTarget {
    fn set_word(&mut self, value: u16, computer: &mut Computer);
    fn set_byte(&mut self, value: u8, computer: &mut Computer);
}

struct VoidWriteTarget {}
impl WriteTarget for VoidWriteTarget {
    fn set_word(&mut self, _value: u16, _computer: &mut Computer) {}
    fn set_byte(&mut self, _value: u8, _computer: &mut Computer) {}
}

#[derive(Copy, Clone)]
struct RegisterWriteTarget {
    register: u8
}
#[allow(dead_code)]
impl RegisterWriteTarget {
    fn new(reg: u8) -> WriteTargets {
        return WriteTargets::REGISTER(RegisterWriteTarget {
            register: reg
        });
    }

    fn new_boxed(reg: u8) -> Box<WriteTargets> {
        return Box::new(Self::new(reg));
    }
}
impl WriteTarget for RegisterWriteTarget {
    fn set_word(&mut self, value: u16, computer: &mut Computer) {
        computer.get_register(self.register).set_word(value);
    }

    fn set_byte(&mut self, value: u8, computer: &mut Computer) {
        computer.get_register(self.register).set_byte(value);
    }
}

#[derive(Copy, Clone)]
struct MemoryWriteTarget {
    address: u16
}
#[allow(dead_code)]
impl MemoryWriteTarget {
    fn new(address: u16) -> WriteTargets {
        return WriteTargets::MEMORY(MemoryWriteTarget {
            address
        });
    }

    fn new_boxed(address: u16) -> Box<WriteTargets> {
        return Box::new(Self::new(address));
    }
}
impl WriteTarget for MemoryWriteTarget {
    fn set_word(&mut self, value: u16, computer: &mut Computer) {
        computer.memory.set_word(self.address, value);
    }

    fn set_byte(&mut self, value: u8, computer: &mut Computer) {
        computer.memory.set_byte(self.address, value);
    }
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
enum WriteTargets {
    VOID,
    REGISTER(RegisterWriteTarget), 
    MEMORY(MemoryWriteTarget)
}

impl<'a> WriteTarget for WriteTargets {
    fn set_word(&mut self, value: u16, computer: &mut Computer) {
        match self {
            WriteTargets::VOID => {},
            WriteTargets::REGISTER(t) => t.set_word(value, computer),
            WriteTargets::MEMORY(t) => t.set_word(value, computer),
        }
    }

    fn set_byte(&mut self, value: u8, computer: &mut Computer) {
        match self {
            WriteTargets::VOID => {},
            WriteTargets::REGISTER(t) => t.set_byte(value, computer),
            WriteTargets::MEMORY(t) => t.set_byte(value, computer),
        }
    }
}

#[allow(dead_code, non_upper_case_globals)]
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
enum SingleOperandOpcodes {
    RRC,
    SWPB,
    RRA,
    SXT,
    PUSH,
    CALL,
    RETI
}

#[allow(dead_code, non_upper_case_globals)]
#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
enum DoubleOperandOpcodes {
    MOV,
    ADD,
    ADDC,
    SUBC,
    SUB,
    CMP,
    DADD,
    BIT,
    BIC,
    BIS,
    XOR,
    AND
}

struct Computer {
    numbered_registers: [BasicRegister; 12],
    memory: MemoryMap,
    pc: EvenRegister,
    sp: EvenRegister,
    sr: StatusRegister,
    cg: ConstantGeneratorRegister
}

#[allow(dead_code)]
impl Computer {
    fn new() -> Computer {
        let pc: EvenRegister = EvenRegister::new(0);
        let sp: EvenRegister = EvenRegister::new(1);
        let sr: StatusRegister = StatusRegister::new();
        let cg: ConstantGeneratorRegister = ConstantGeneratorRegister::new();
        let numbered_registers: &mut [BasicRegister; 12] = &mut [BasicRegister::new(255); 12];
        for i in 4..16u8 {
            numbered_registers[i as usize - 4] = BasicRegister::new(i);
        }
        return Computer {
            numbered_registers: *numbered_registers,
            memory: MemoryMap::new(),
            pc, sp, sr, cg
        };
    }

    fn reset(&mut self) {
        self.memory.reset();
        self.pc.set_word(0);
        self.sp.set_word(0);
        self.sr.set_word(0);
        self.cg.set_word(0);

        for i in 0..12 {
            self.numbered_registers[i].set_word(0);
        }
    }

    fn get_register(&mut self, id: u8) -> &mut dyn RegisterData {
        if id == 0 {
            return &mut self.pc;
        } else if id == 1 {
            return &mut self.sp;
        } else if id == 2 {
            return &mut self.sr;
        } else if id == 3 {
            return &mut self.cg;
        } else {
            return &mut self.numbered_registers[(id - 4) as usize];
        }
    }

    fn get_register_imut(&self, id: u8) -> &dyn RegisterData {
        if id == 0 {
            return &self.pc;
        } else if id == 1 {
            return &self.sp;
        } else if id == 2 {
            return &self.sr;
        } else if id == 3 {
            return &self.cg;
        } else {
            return &self.numbered_registers[(id - 4) as usize];
        }
    }

    fn interrupt(&mut self, id: u16) {
        if self.sr.get_status(StatusFlags::GIE) { // only actually interrupt if interrupts are enabled
            // push PC and SR onto the stack for restoring after the interrupt handler
            self._push(self.pc.get_word(), false);
            self._push(self.sr.get_word(), false);
            // clear status register (setting GIE to 0)
            self.sr.set_word(0);
            // load interrupt vector into pc
            self.pc.set_word(self.memory.get_word(id));
        }
    }

    fn step(&mut self) {
        if self.sr.get_status(StatusFlags::CPUOFF) {
            return;
        }
        let pc_w: u16 = self.pc.get_word();
        let instruction: u16 = self.memory.get_word(pc_w);
        self.pc.set_word(pc_w + 2);

        self._execute(instruction);
    }

    fn _execute(&mut self, instruction: u16) {
        if instruction >> 10 == 4 { // 0b000100
            // single operand instruction
            self._execute_single_operand(instruction);
        } else if instruction >> 13 == 1 { // 0b001
            // jump instruction
            self._execute_jump(instruction);
        } else if instruction != 0 {
            // double operand instruction
            self._execute_double_operand(instruction);
        }
    }

    fn _execute_jump(&mut self, instruction: u16) { // all of this is tested
        let offset: &mut i32 = &mut ((instruction as i32) & 0x3ff);
        if *offset > 512 {
            *offset -= 1024;
        }
        let condition: u8 = ((instruction >> 10) & 0x7) as u8;
        match condition {
            0 => { // JNE/JNZ
                if self.sr.get_status(StatusFlags::ZERO) {return;}
            },
            1 => { // JEQ/JZ
                if !self.sr.get_status(StatusFlags::ZERO) {return;}
            },
            2 => { // JNC/JLO
                if self.sr.get_status(StatusFlags::CARRY) {return;}
            },
            3 => { // JC/JHS
                if !self.sr.get_status(StatusFlags::CARRY) {return;}
            },
            4 => { // JN
                if !self.sr.get_status(StatusFlags::NEGATIVE) {return;}
            },
            5 => { // JGE
                if self.sr.get_status(StatusFlags::NEGATIVE) ^ self.sr.get_status(StatusFlags::OVERFLOW) {return;}
            },
            6 => { // JL
                if !(self.sr.get_status(StatusFlags::NEGATIVE) ^ self.sr.get_status(StatusFlags::OVERFLOW)) {return;}
            },
            7 => { // JMP
                // unconditional jump
            }
            _ => println!("Unknown condition"),
        }

        self.pc.set_word((self.pc.get_word() as i32 + (*offset * 2)) as u16);
    }

    fn _get_src(&mut self, src_reg: u8, as_: u8, bw: bool) -> (u16, Box<WriteTargets>) {
        let src: &mut u16 = &mut 0;
        if src_reg == 3 || (src_reg == 2 && as_ > 1) { // CG (or SR outside of Register or Indexed modes)
            if src_reg == 2 {
                if as_ == 2 {
                    *src = 4;
                } else if as_ == 3 {
                    *src = 8;
                }
            } else if src_reg == 3 {
                if as_ == 0 {
                    *src = 0;
                } else if as_ == 1 {
                    *src = 1;
                } else if as_ == 2 {
                    *src = 2;
                } else if as_ == 3 {
                    *src = if bw {0xff} else {0xffff};
                }
            }
            return (*src, Box::new(WriteTargets::VOID));
        }
        
        if as_ == 0 { // Register Mode
            if bw {
                *src = self.get_register(src_reg).get_byte() as u16;
            } else {
                *src = self.get_register(src_reg).get_word();
            }
            return (*src, RegisterWriteTarget::new_boxed(src_reg));
        } else if as_ == 1 { // Indexed Mode
            let offset: u16;
            if src_reg == 2 { // Special-Case Absolute Mode
                offset = self.memory.get_word(self.pc.get_word()); // not adding src reg
            } else {
                offset = self.memory.get_word(self.pc.get_word()).wrapping_add(self.get_register(src_reg).get_word());
            }
            self.pc.set_word(self.pc.get_word().wrapping_add(2));
            *src = if bw {self.memory.get_byte(offset) as u16} else {self.memory.get_word(offset)};
            return (*src, MemoryWriteTarget::new_boxed(offset));
        } else if as_ == 2 { // Register Indirect Mode
            let target: u16 = self.get_register(src_reg).get_word();
            *src = if bw {self.memory.get_byte(target) as u16} else {self.memory.get_word(target)};
            return (*src, MemoryWriteTarget::new_boxed(target));
        } else if as_ == 3 { // Register Indirect Autoincrement Mode
            let mem_target: u16 = self.get_register(src_reg).get_word();
            if bw {
                *src = self.memory.get_byte(mem_target) as u16;
                let extra: u16 = (src_reg == 0 || src_reg == 1) as u16; // PC or SP
                self.get_register(src_reg).set_word(mem_target.wrapping_add(1).wrapping_add(extra));
            } else {
                *src = self.memory.get_word(mem_target);
                self.get_register(src_reg).set_word(mem_target.wrapping_add(2));
            }
            return (*src, MemoryWriteTarget::new_boxed(mem_target));
        } else {
            panic!("Impossible source addressing mode");
        }
    }

    fn _push(&mut self, value: u16, bw: bool) {
        let mut sp_word: u16 = self.sp.get_word();
        if sp_word <= 1 {
            sp_word += 0xffff - 2;
        } else {
            sp_word -= 2;
        }
        self.sp.set_word(sp_word);
        if bw {
            self.memory.set_byte(sp_word+1, (value & 0xff) as u8);
        } else {
            self.memory.set_word(sp_word, value);
        }
    }

    fn _execute_single_operand(&mut self, instruction: u16) { // PUSH implementation: decrement SP,
                                                              // then execute as usual
        let opcode: u8 = ((instruction >> 7) & 0x7) as u8; // 3-bit (0b111)
        let src_reg: u8 = (instruction & 0xf) as u8;       // 4-bit (0b1111)
        let as_: u8 = ((instruction >> 4) & 0x3) as u8;    // 2-bit (0b11)
        let bw: bool = (instruction >> 6) & 0x1 == 1;
        let bw_num: u16 = if bw {7} else {15};

        // read source
        let (src_imu, mut wt) = self._get_src(src_reg, as_, bw);
        let src: &mut u16 = &mut 0;
        *src = src_imu;

        let no_write: &mut bool = &mut false;
        
        // apply operation
        let opc: SingleOperandOpcodes = SingleOperandOpcodes::try_from(opcode).unwrap();
        
        match opc {
            SingleOperandOpcodes::RRC => { // tested
                let carry: bool = (*src & 1) == 1;
                *src >>= 1;
                // put carry back in, taking into account byte-mode as bw
                *src |= (self.sr.get_status(StatusFlags::CARRY) as u16) << bw_num;

                self.sr.set_status(StatusFlags::CARRY, carry);
                self.sr.set_status(StatusFlags::NEGATIVE, (*src >> bw_num & 1) == 1);
                self.sr.set_status(StatusFlags::ZERO, *src == 0);
                self.sr.set_status(StatusFlags::OVERFLOW, false);
            },
            SingleOperandOpcodes::SWPB => { // tested
                if !bw {
                    *src = ((*src & 0xff00) >> 8) | ((*src & 0xff) << 8);
                }
            },
            SingleOperandOpcodes::RRA => { // tested
                self.sr.set_status(StatusFlags::CARRY, *src & 1 == 1);
                let msb_to_or: u16 = *src & (if bw {128} else {32768});
                *src >>= 1;
                *src |= msb_to_or;
                self.sr.set_status(StatusFlags::NEGATIVE, (*src >> bw_num) & 1 == 1);
                self.sr.set_status(StatusFlags::ZERO, *src == 0);
                self.sr.set_status(StatusFlags::OVERFLOW, false);
            },
            SingleOperandOpcodes::SXT => { // tested
                if !bw {
                    *src &= 0xff;
                    if (*src >> 7 & 1) == 1 {
                        *src |= 0xff00;
                        self.sr.set_status(StatusFlags::NEGATIVE, true);
                    } else {
                        self.sr.set_status(StatusFlags::NEGATIVE, false);
                    }
                    self.sr.set_status(StatusFlags::ZERO, *src == 0);
                    self.sr.set_status(StatusFlags::CARRY, *src != 0);
                    self.sr.set_status(StatusFlags::OVERFLOW, false);
                }
            },
            SingleOperandOpcodes::PUSH => { // tested (indirectly) by other tests
                self._push(*src, bw);
                *no_write = true;
            },
            SingleOperandOpcodes::CALL => { // tested
                if !bw {
                    self.sp.set_word(self.sp.get_word().wrapping_sub(2));
                    self.memory.set_word(self.sp.get_word(), self.pc.get_word());
                    self.pc.set_word(*src);
                    *no_write = true;
                }
            },
            SingleOperandOpcodes::RETI => { // tested
                // pop SR
                self.sr.set_word(self.memory.get_word(self.sp.get_word()));
                self.sp.set_word(self.sp.get_word() + 2);

                // pop PC
                self.pc.set_word(self.memory.get_word(self.sp.get_word()));
                self.sp.set_word(self.sp.get_word() + 2);
            }
        }

        if !(*no_write) {
            if bw {
                wt.set_byte((*src & 0xff) as u8, self);
            } else {
                wt.set_word(*src, self);
            }
        }
    }

    fn _set_flags(&mut self, src: u16, prev_dst: u16, full_dst: u32, dst: u16, byte_mode: bool) {
        let byte_int: u16 = if byte_mode {7} else {15};
        let dst_sign: u16 = dst >> byte_int & 1;
        let prev_dst_sign: u16 = prev_dst >> byte_int & 1;
        self.sr.set_status(StatusFlags::ZERO, dst == 0);
        self.sr.set_status(StatusFlags::NEGATIVE, dst_sign == 1);
        self.sr.set_status(StatusFlags::CARRY, full_dst > (if byte_mode {0xff} else {0xffff}));
        // overflow is set if the sign of the operands is the same, and the sign of the result is different
        // (e.g. positive + positive = negative, or negative + negative = positive)
        self.sr.set_status(StatusFlags::OVERFLOW, (prev_dst == (src >> byte_int & 1)) && (prev_dst_sign != dst_sign));
    }

    fn _execute_double_operand(&mut self, instruction: u16) {
        let opcode: u8 = ((instruction >> 12) & 0xf) as u8; // 4-bit
        let src_reg: u8 = ((instruction >> 8) & 0xf) as u8; // 4-bit
        let ad: u8 = ((instruction >> 7) & 0x1) as u8;      // 1-bit
        let bw: bool = ((instruction >> 6) & 0x1) == 1;     // 1-bit
        let as_: u8 = ((instruction >> 4) & 0x3) as u8;     // 2-bit
        let dst_reg: u8 = (instruction & 0xf) as u8;        // 4-bit
        let byte_int: u16 = if bw {7} else {15};

        // read source
        let (src, _) = self._get_src(src_reg, as_, bw);

        let dst: &mut u16 = &mut 0;
        let wt: &mut WriteTargets = &mut WriteTargets::VOID;
        // read value of dst and make a write target
        if ad == 0 {
            if bw {
                *dst = self.get_register(dst_reg).get_byte() as u16;
            } else {
                *dst = self.get_register(dst_reg).get_word();
            }
            *wt = RegisterWriteTarget::new(dst_reg);
        } else {
            let offset: u16 = self.memory.get_word(self.pc.get_word()) + self.get_register(dst_reg).get_word();
            self.pc.set_word(self.pc.get_word() + 2);
            if bw {
                *dst = self.memory.get_byte(offset) as u16;
            } else {
                *dst = self.memory.get_word(offset);
            }
            *wt = MemoryWriteTarget::new(offset);
        }

        let no_write: &mut bool = &mut false;

        //println!("opcode: {}", opcode);
        let opc: DoubleOperandOpcodes = DoubleOperandOpcodes::try_from(opcode - 4).unwrap();
        //println!("opc: {:#?}", opc);

        let cutoff: u32 = if bw {0xff} else {0xffff};

        match opc {
            DoubleOperandOpcodes::MOV => { // tested
                *dst = src;
            },
            DoubleOperandOpcodes::ADD => { // tested
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32) + (src as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::ADDC => { // tested
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32) + (src as u32) + (self.sr.get_status(StatusFlags::CARRY) as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::SUBC => { // Fuzzed
                let prev_dst: u16 = *dst;
                // dst - src - 1 + sr(CARRY)
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32)
                    .wrapping_sub(1).wrapping_add(self.sr.get_status(StatusFlags::CARRY) as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::SUB => { // tested & fuzzed
                let prev_dst: u16 = *dst;
                //println!("SUB running {} - {}", *dst, src);
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::CMP => { // not tested, but same impl as SUB
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32);
                let fake_dst: u16 = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, fake_dst, bw);
                *no_write = true;
            },
            DoubleOperandOpcodes::DADD => { // Doesn't need testing
                panic!("AHhhhhhhhhhhhhhhhhhhh I have no clue how DADD works.");
            },
            DoubleOperandOpcodes::BIT => { // not tested, but same impl as AND
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst & src) as u32;
                let fake_dst: u16 = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, fake_dst, bw);
                self.sr.set_status(StatusFlags::CARRY, !self.sr.get_status(StatusFlags::ZERO));
                self.sr.set_status(StatusFlags::OVERFLOW, false);
                *no_write = true;
            },
            DoubleOperandOpcodes::BIC => { // tested
                *dst &= !src;
            },
            DoubleOperandOpcodes::BIS => { // tested
                *dst |= src;
            },
            DoubleOperandOpcodes::XOR => { // tested
                let prev_dst: u16 = *dst;
                *dst ^= src;
                self.sr.set_status(StatusFlags::NEGATIVE, (*dst >> byte_int & 1) == 1);
                self.sr.set_status(StatusFlags::ZERO, *dst == 0);
                self.sr.set_status(StatusFlags::CARRY, *dst != 0);
                self.sr.set_status(StatusFlags::OVERFLOW, (src >> byte_int & 1) == 1 && (prev_dst >> byte_int & 1) == 1);
            },
            DoubleOperandOpcodes::AND => { // tested
                *dst &= src;
                self.sr.set_status(StatusFlags::NEGATIVE, (*dst >> byte_int & 1) == 1);
                self.sr.set_status(StatusFlags::ZERO, *dst == 0);
                self.sr.set_status(StatusFlags::CARRY, *dst != 0);
                self.sr.set_status(StatusFlags::OVERFLOW, false);
            },
        }
        if !(*no_write) {
            if bw {
                wt.set_byte((*dst & 0xff) as u8, self);
            } else {
                wt.set_word(*dst, self);
            }
        }
    }
}

fn file_as_byte_vec(filename: &String) -> Vec<u8> {
    println!("Decoding file: '{}'", filename);
    let mut f = File::open(&filename).expect("File not found");
    let mut buf: Vec<u8> = Vec::new();
    f.read_to_end(&mut buf).expect("Failed to read file");
    return buf;
}

#[derive(Debug)]
enum ShmemCommands {
    None,
    Stop,
    Run,
    Step(u16),
    LoadFile(String),
    SetMem(u16, u16),
    Interrupt(u16),
    Unknown
}

enum RunMode {
    Stopped,
    Running,
    Stepping(u16)
}

struct SharedMemorySystem {
    raw_ptr: *mut u8
}
impl SharedMemorySystem {
    fn new(raw_ptr: *mut u8) -> SharedMemorySystem {
        return SharedMemorySystem { raw_ptr };
    }

    fn write_byte(&mut self, idx: usize, value: u8) {
        if idx >= 0x10420 {
            panic!("Index error in write byte, {} is more than 65 kb", idx);
        }
        unsafe {
            std::ptr::write_volatile(self.raw_ptr.add(idx), value);
        }
    }

    fn read_byte(&self, idx: usize) -> u8 {
        if idx >= 0x10420 {
            panic!("Index error in read byte, {} is more than 65 kb", idx);
        }
        unsafe {
            return std::ptr::read_volatile(self.raw_ptr.add(idx));
        }
    }

    fn read_string(&self, idx: usize) -> String {
        if idx >= 0x10420 {
            panic!("Index error in read byte, {} is more than 65 kb", idx);
        }
        let c_buf: *const c_char = unsafe { self.raw_ptr.add(idx) } as *const c_char;
        let c_str: &CStr = unsafe { CStr::from_ptr(c_buf) };
        return c_str.to_str().unwrap().to_owned();
    }

    fn write(&mut self, computer: &Computer) {
        for i in 0..=0xffffu16 {
            self.write_byte(i as usize, computer.memory.get_byte(i));
        }
        for i in 0..=15 {
            let reg_val: u16 = computer.get_register_imut(i).get_word();
            let high: u8 = ((reg_val & 0xff00) >> 8) as u8;
            let low: u8 = (reg_val & 0xff) as u8;
            self.write_byte((i as usize)*2 + 0x10000, high);
            self.write_byte((i as usize)*2 + 0x10000 + 1 , low);
        }
    }

    fn get_command(&self) -> ShmemCommands {
        const CMD: usize = 0x10020;
        let cmd_id = self.read_byte(CMD);

        return match cmd_id {
            0 => ShmemCommands::None,
            1 => ShmemCommands::Stop,
            2 => ShmemCommands::Run,
            3 => {
                let high: u16 = self.read_byte(CMD + 1) as u16;
                let low: u16 = self.read_byte(CMD + 2) as u16;
                return ShmemCommands::Step((high << 8) | low);
            },
            4 => {
                return ShmemCommands::LoadFile(self.read_string(CMD + 1));
            },
            5 => {
                let high_addr: u16 = self.read_byte(CMD + 1) as u16;
                let low_addr: u16 = self.read_byte(CMD + 2) as u16;
                let high_val: u16 = self.read_byte(CMD + 3) as u16;
                let low_val: u16 = self.read_byte(CMD + 4) as u16;
                return ShmemCommands::SetMem((high_addr << 8) | low_addr, (high_val << 8) | low_val);
            },
            6 => {
                let high: u16 = self.read_byte(CMD + 1) as u16;
                let low: u16 = self.read_byte(CMD + 2) as u16;
                return ShmemCommands::Interrupt((high << 8) | low);
            },
            _ => ShmemCommands::Unknown
        };
    }

    fn acknowledge_command(&mut self) {
        const CMD: usize = 0x10020;
        self.write_byte(CMD, 0);
    }
}

fn actually_run(running: Arc<AtomicBool>) {
    let shmem_path = std::env::temp_dir().join("msp430_shmem_id");
    let shmem_flink: &str = shmem_path.to_str().expect("Failed to get shared memory path");
    // Create or open the shared memory mapping
    let mut shmem = match ShmemConf::new().size(0x10420).flink(shmem_flink).create() {
        Ok(m) => m,
        Err(ShmemError::LinkExists) => {
            eprintln!("Shared memory already exists, make sure msp430_rust is not already running");
            return;
            //ShmemConf::new().flink(shmem_flink).open().unwrap()
        },
        Err(e) => {
            eprintln!(
                "Unable to create or open shmem flink {} : {}",
                shmem_flink, e
            );
            return;
        }
    };
    shmem.set_owner(true);

    #[cfg(debug_assertions)]
    println!("Shared memory id: {}", shmem.get_os_id());
    #[cfg(debug_assertions)]
    println!("Shared memory id shared at: {}", shmem_flink);

    // Get pointer to the shared memory
    let raw_ptr: *mut u8 = shmem.as_ptr();

    let mut mem = SharedMemorySystem::new(raw_ptr);

    let mut run_mode: RunMode = RunMode::Stopped;

    let c: &mut Computer = &mut Computer::new();
    let mut iters: u128 = 0;
    const CHECK_EVERY: u128 = 1_000_000;

    while running.load(Ordering::SeqCst) { // ensure that shared memory is properly
                                           // dropped before exit
        let mut handle_commands: bool = false;
        match run_mode {
            RunMode::Stopped => handle_commands = true,
            RunMode::Running => {
                c.step();
                iters += 1;
            },
            RunMode::Stepping(count) => {
                if count <= 1 {
                    run_mode = RunMode::Stopped;
                } else {
                    run_mode = RunMode::Stepping(count - 1);
                }
                c.step();
                iters += 1;
            }
        }
        if handle_commands || iters > CHECK_EVERY {
            iters = 0;
            let cmd = &mem.get_command();

            match cmd {
                ShmemCommands::None => {
                    mem.write(c);
                    continue;
                },
                ShmemCommands::Stop => run_mode = RunMode::Stopped,
                ShmemCommands::Run => run_mode = RunMode::Running,
                ShmemCommands::Step(n) => run_mode = RunMode::Stepping(*n),
                ShmemCommands::LoadFile(path) => {
                    c.reset();
                    run_mode = RunMode::Stopped;
                    let buf: Vec<u8> = file_as_byte_vec(path);
                    // load program into computer
                    utils::execute_nr_nd(c, &buf, 0);
                    #[cfg(debug_assertions)]
                    println!("Computer pc: {}", c.get_register_imut(0).get_word());
                },
                &ShmemCommands::SetMem(addr, val) => {
                    c.memory.set_word(addr, val);
                },
                &ShmemCommands::Interrupt(vector) => {
                    c.interrupt(vector);
                },
                ShmemCommands::Unknown => {},
            };
            
            mem.acknowledge_command();
            mem.write(c);
            #[cfg(debug_assertions)]
            println!("Handled command: {:#?}", cmd);
        }
    }
}

fn run_wrapper() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    actually_run(running);
}

fn fork_and_run() {
    let result = daemon(false, true);
    match result {
        Ok(Fork::Child) => run_wrapper(),
        Ok(Fork::Parent(child_pid)) => println!("{}", child_pid),
        Err(_) => println!("Failed to fork"),
    }
    /*if let Ok(Fork::Parent(_)) = daemon(true, true) {
        run_wrapper();
    }*/
}

fn main() {
    let args: CLI = CLI::parse();

    match args {
        CLI::Benchmark => run_benchmarks(),
        CLI::Run => run_wrapper(),
        CLI::RunForked => fork_and_run(),
    }
}

fn run_benchmarks() {
    let rounds = 1_000_000;
    let steps = 500;
    let mut time_elapsed: u128 = 0;
    
    println!("Running {} rounds of {} steps each...", rounds, steps);
    let assembled = utils::assemble(r#"
.define "r5" A
.define "r6" B
.define "r15" OUT
mov #0 [A]
mov #1 [B]
mov #0x4400 sp

loop:
add [A] [B] ; add value of A into B
mov [B] [OUT] ; copy value of B into OUT
add [B] [A] ; add value of B into A
mov [A] [OUT] ; copy value of A into OUT
jmp loop
"#);
    let trimmed = assembled.trim();

    for _ in 0..rounds {
        let c: &mut Computer = &mut Computer::new();
        utils::execute(c, trimmed, 0);
        let start = Instant::now();
        for _ in 0..steps {
            c.step();
        }
        let elapsed = start.elapsed();
        time_elapsed += elapsed.as_micros();
    }
    let micros_per_cycle: f64 = (time_elapsed as f64) / (rounds as f64) / (steps as f64);
    let hz = 1000000.0 / micros_per_cycle;
    let khz = hz / 1000.0;
    let mhz = khz / 1000.0;

    println!("{} us/cycle ({} Hz, {} KHz, {} MHz)", micros_per_cycle, hz, khz, mhz);
}

#[cfg(test)]
mod tests;

pub(crate) mod utils;

/*
fn main() {
    println!("Hello, world!");

    print!("What's your name? ");
    io::stdout().flush().unwrap();

    let name: &mut String = &mut String::new();
    let result: Result<usize, std::io::Error> = io::stdin().read_line(name);
    match result {
        Ok(_) => greet(name),
        Err(_) => println!("There was an error during input")
    };
}

fn greet(name: &String) {
    println!("Hello, {}!", &name.trim());
}*/
