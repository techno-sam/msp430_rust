use bitflags::bitflags;
use num_enum::TryFromPrimitive;

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
        return ((self._memory[index as usize] as u16) << 8u16) + (self._memory[index as usize + 1] as u16);
    }

    fn set_word(&mut self, index: u16, value: u16) {
        //assert_eq!(index % 2, 0);
        self._memory[index as usize] = ((value >> 8) & 0xff) as u8;
        self._memory[index as usize + 1] = (value & 0xff) as u8;
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
            t => t.set_byte(value, computer),
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
        let pc: EvenRegister = EvenRegister::new(0); // TODO: PC needs something to be word-aligned
        let sp: EvenRegister = EvenRegister::new(1); // TODO: ditto
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

    fn step(&mut self) {
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

    fn _execute_jump(&mut self, instruction: u16) {
        let offset: &mut i32 = &mut (instruction as i32);
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
                offset = self.memory.get_word(self.pc.get_word()); // NOTE: not adding src reg
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
            SingleOperandOpcodes::RRC => { // NOTE: tested
                let carry: bool = (*src & 1) == 1;
                *src >>= 1;
                // put carry back in, taking into account byte-mode as bw
                *src |= (self.sr.get_status(StatusFlags::CARRY) as u16) << bw_num;

                self.sr.set_status(StatusFlags::CARRY, carry);
                self.sr.set_status(StatusFlags::NEGATIVE, (*src >> bw_num & 1) == 1);
                self.sr.set_status(StatusFlags::ZERO, *src == 0);
                self.sr.set_status(StatusFlags::OVERFLOW, false);
            },
            SingleOperandOpcodes::SWPB => { // NOTE: tested
                if !bw {
                    *src = ((*src & 0xff00) >> 8) | ((*src & 0xff) << 8);
                }
            },
            SingleOperandOpcodes::RRA => { // NOTE: tested
                self.sr.set_status(StatusFlags::CARRY, *src & 1 == 1);
                let msb_to_or: u16 = *src & (if bw {128} else {32768});
                *src >>= 1;
                *src |= msb_to_or;
                self.sr.set_status(StatusFlags::NEGATIVE, (*src >> bw_num) & 1 == 1);
                self.sr.set_status(StatusFlags::ZERO, *src == 0);
                self.sr.set_status(StatusFlags::OVERFLOW, false);
            },
            SingleOperandOpcodes::SXT => { // NOTE: tested
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
            SingleOperandOpcodes::PUSH => { // NOTE: tested (indirectly) by other tests
                let mut sp_word: u16 = self.sp.get_word();
                match *wt {
                    WriteTargets::REGISTER(wt_reg) => {
                        if wt_reg.register == 0 { // PC
                            if bw {
                                *src = self.pc.get_byte() as u16;
                            } else {
                                *src = sp_word;
                            }
                        }
                    },
                    _ => {}, // intentionally left blank
                }
                if sp_word <= 1 {
                    //panic!("MSP430 CPU Stack overflow");
                    sp_word += 0xffff - 2;
                } else {
                    sp_word -= 2;
                }
                self.sp.set_word(sp_word);
                *no_write = true;
                if bw {
                    self.memory.set_byte(sp_word, (*src & 0xff) as u8);
                } else {
                    self.memory.set_word(sp_word, *src);
                }
            },
            SingleOperandOpcodes::CALL => { // TODO: test
                if !bw {
                    self.sp.set_word(self.sp.get_word().wrapping_sub(2));
                    self.memory.set_word(self.sp.get_word(), self.pc.get_word());
                    self.pc.set_word(*src);
                    *no_write = true;
                }
            },
            SingleOperandOpcodes::RETI => { // TODO: IMPLEMENT INTERRUPTS
                todo!();
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
            DoubleOperandOpcodes::MOV => { // NOTE: tested
                *dst = src;
            },
            DoubleOperandOpcodes::ADD => { // NOTE: tested
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32) + (src as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::ADDC => { // NOTE: tested
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32) + (src as u32) + (self.sr.get_status(StatusFlags::CARRY) as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::SUBC => { // NOTE: Fuzzed
                let prev_dst: u16 = *dst;
                // dst - src - 1 + sr(CARRY)
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32)
                    .wrapping_sub(1).wrapping_add(self.sr.get_status(StatusFlags::CARRY) as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::SUB => { // NOTE: tested & fuzzed
                let prev_dst: u16 = *dst;
                //println!("SUB running {} - {}", *dst, src);
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32);
                *dst = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, *dst, bw);
            },
            DoubleOperandOpcodes::CMP => { // NOTE: not tested, but same impl as SUB
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst as u32).wrapping_sub(src as u32);
                let fake_dst: u16 = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, fake_dst, bw);
                *no_write = true;
            },
            DoubleOperandOpcodes::DADD => { // NOTE: Doesn't need testing
                panic!("AHhhhhhhhhhhhhhhhhhhh I have no clue how DADD works.");
            },
            DoubleOperandOpcodes::BIT => { // NOTE: not tested, but same impl as AND
                let prev_dst: u16 = *dst;
                let full_dst: u32 = (*dst & src) as u32;
                let fake_dst: u16 = (full_dst & cutoff) as u16;
                self._set_flags(src, prev_dst, full_dst, fake_dst, bw);
                self.sr.set_status(StatusFlags::CARRY, !self.sr.get_status(StatusFlags::ZERO));
                self.sr.set_status(StatusFlags::OVERFLOW, false);
                *no_write = true;
            },
            DoubleOperandOpcodes::BIC => { // NOTE: tested
                *dst &= !src;
            },
            DoubleOperandOpcodes::BIS => { // NOTE: tested
                *dst |= src;
            },
            DoubleOperandOpcodes::XOR => { // NOTE: tested
                let prev_dst: u16 = *dst;
                *dst ^= src;
                self.sr.set_status(StatusFlags::NEGATIVE, (*dst >> byte_int & 1) == 1);
                self.sr.set_status(StatusFlags::ZERO, *dst == 0);
                self.sr.set_status(StatusFlags::CARRY, *dst != 0);
                self.sr.set_status(StatusFlags::OVERFLOW, (src >> byte_int & 1) == 1 && (prev_dst >> byte_int & 1) == 1);
            },
            DoubleOperandOpcodes::AND => { // NOTE: tested
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


fn main() {
    println!("Shared memory!");

    let a = StatusFlags::CARRY | StatusFlags::CPUOFF;
    println!("The following should be true:");
    println!("{:?}", a.contains(StatusFlags::CARRY));
    println!("The following should be false:");
    println!("{:#?}", a.contains(StatusFlags::ZERO));

    println!("The following should be true:");
    println!("{:?}", (0x003 & StatusFlags::ZERO.to_owned().bits()) != 0);

    let reg: &mut dyn RegisterData = &mut BasicRegister {
        id: 4,
        _value: 41
    };

    reg.set_word(4);

    let mm: &mut MemoryMap = &mut MemoryMap::new();

    println!("{:?}", mm._memory[0xffff]);

    let computer: &mut Computer = &mut Computer::new();
    computer.step();
}

#[cfg(test)]
mod tests;


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
