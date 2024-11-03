use super::execute::DecodedInstr;
use crate::State;

#[derive(Debug)]
pub struct Instr(u16);

impl Instr {
    pub fn decode(self) -> DecodedInstr {
        match self.0 {
            0x00E0 => DecodedInstr::ClearScreen,
            0x00EE => DecodedInstr::Return,
            0x1000..=0x1FFF => DecodedInstr::Jump {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0x2000..=0x2FFF => DecodedInstr::Call {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0x3000..=0x3FFF => DecodedInstr::SkipIfEqual {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x4000..=0x4FFF => DecodedInstr::SkipIfNotEqual {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x5000..=0x5FFF if self.0 & 0xF == 0 => DecodedInstr::SkipIfRegisterEqual {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
            },
            0x6000..=0x6FFF => DecodedInstr::LoadRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x7000..=0x7FFF => DecodedInstr::AddToRegister {
                register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                value: (self.0 & 0xFF).try_into().unwrap(),
            },
            0x8000..=0x8FFF => match self.0 & 0xF {
                0x0 => DecodedInstr::CopyRegister {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x1 => DecodedInstr::OrRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x2 => DecodedInstr::AndRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x3 => DecodedInstr::XorRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x4 => DecodedInstr::AddRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x5 => DecodedInstr::SubtractRegisters {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x6 => DecodedInstr::ShiftRight {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x7 => DecodedInstr::SubtractRegistersReverse {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0xE => DecodedInstr::ShiftLeft {
                    x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                    y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                },
                0x8..=0xD | 0xF => DecodedInstr::IllegalInstruction(self.0),
                0x10.. => unreachable!(),
            },
            0x9000..=0x9FFF if self.0 & 0xF == 0 => DecodedInstr::SkipIfRegisterNotEqual {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
            },
            0xA000..=0xAFFF => DecodedInstr::LoadIRegister {
                value: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0xB000..=0xBFFF => DecodedInstr::JumpWithOffset {
                address: (self.0 & 0x0FFF).try_into().unwrap(),
            },
            0xD000..=0xDFFF => DecodedInstr::DrawSprite {
                x: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                y: ((self.0 & 0x00F0) >> 4).try_into().unwrap(),
                bytes: (self.0 & 0x000F).try_into().unwrap(),
            },
            0xE000..=0xEFFF => match u8::try_from(self.0 & 0xFF).unwrap() {
                0x9E => DecodedInstr::SkipIfPressed {
                    key: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0xA1 => DecodedInstr::SkipIfNotPressed {
                    key: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                _ => DecodedInstr::IllegalInstruction(self.0),
            },
            0xF000..=0xFFFF => match u8::try_from(self.0 & 0xFF).unwrap() {
                0x07 => DecodedInstr::StoreDelayTimer {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x0A => DecodedInstr::WaitForKeyPress {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x15 => DecodedInstr::SetDelayTimer {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x1E => DecodedInstr::AddToIRegister {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x33 => DecodedInstr::BinaryCodedDecimal {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x55 => DecodedInstr::StoreRegisters {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                0x65 => DecodedInstr::LoadRegisters {
                    register: ((self.0 & 0x0F00) >> 8).try_into().unwrap(),
                },
                _ => DecodedInstr::IllegalInstruction(self.0),
            },
            _ => DecodedInstr::IllegalInstruction(self.0),
        }
    }
}

impl State {
    pub fn fetch(&self) -> Instr {
        Instr(u16::from_be_bytes([
            self.memory[self.pc],
            self.memory[self.pc + 1],
        ]))
    }
}
