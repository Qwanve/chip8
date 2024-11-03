use core::ops::Index;
use core::ops::IndexMut;
use core::pin::pin;
use core::time::Duration;
use futures::select;
use futures::FutureExt;
use log::*;
use smol::Timer;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::Mutex;
use ux::u4;

mod instruction;
mod io;

fn main() {
    env_logger::init();
    let vram = Arc::new(Mutex::<[bool; 64 * 32]>::new([false; 64 * 32]));
    let keypad = Arc::new(Mutex::new(io::Keypad([false; 16])));
    let delay_timer = Arc::new(Mutex::new(0));
    let sound_timer = Arc::new(Mutex::new(0));
    let file = std::env::args()
        .nth(1)
        .expect("Expected rom as first arguement");
    info!("Opening rom");
    let rom = std::fs::read(file).unwrap();
    let mut state = State::new(
        vram.clone(),
        keypad.clone(),
        delay_timer.clone(),
        sound_timer.clone(),
        rom,
    );
    let mut disp = pin!(io::sdl2(vram.clone(), keypad.clone(), sound_timer.clone()).fuse());
    smol::block_on(async {
        select! {
            _ = disp => return,
            _ = handle_timer(sound_timer).fuse() => {},
            _ = handle_timer(delay_timer).fuse() => {},
            reason = state.run().fuse() => error!("Core returned: {reason:?}"),
        };
        disp.await;
    });
}

#[derive(Clone)]
struct Memory {
    rom: Vec<u8>,
}

impl Index<u16> for Memory {
    type Output = u8;
    fn index(&self, idx: u16) -> &Self::Output {
        //TODO: Fonts
        trace!("Accessing memory {idx:#X}");
        match idx {
            0x1FF => &0,
            0x200.. => {
                let idx = usize::from(idx) - 0x200;
                self.rom.get(idx).unwrap_or(&0)
            }
            _ => todo!(),
        }
    }
}
impl IndexMut<u16> for Memory {
    fn index_mut(&mut self, idx: u16) -> &mut Self::Output {
        trace!("Accessing memory {idx:#X}");
        //TODO: Fonts
        match idx {
            0x200.. => {
                let idx = usize::from(idx) - 0x200;
                if self.rom.len() <= idx {
                    self.rom.resize_with(idx + 1, Default::default);
                }
                &mut self.rom[idx]
            }
            _ => todo!(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ExitReason {
    InfiniteLoop,
    WaitingForKeyPress,
    IllegalInstruction,
}

#[derive(Clone)]
struct Registers([u8; 16]);

impl Index<u4> for Registers {
    type Output = u8;
    fn index(&self, idx: u4) -> &Self::Output {
        trace!("Accessing register {idx:#X}");
        &self.0[usize::from(u8::from(idx))]
    }
}

impl IndexMut<u4> for Registers {
    fn index_mut(&mut self, idx: u4) -> &mut Self::Output {
        trace!("Accessing register {idx:#X}");
        &mut self.0[usize::from(u8::from(idx))]
    }
}

#[derive(Clone)]
struct State {
    pc: u16,
    vram: Arc<Mutex<[bool; 64 * 32]>>,
    memory: Memory,
    stack: Vec<u16>,
    registers: Registers,
    vi: u16,
    keypad: Arc<Mutex<io::Keypad>>,
    delay_timer: Arc<Mutex<u8>>,
    sound_timer: Arc<Mutex<u8>>,
    last_key_press: Option<u8>,
}
impl State {
    fn new(
        vram: Arc<Mutex<[bool; 64 * 32]>>,
        keypad: Arc<Mutex<io::Keypad>>,
        delay_timer: Arc<Mutex<u8>>,
        sound_timer: Arc<Mutex<u8>>,
        rom: Vec<u8>,
    ) -> State {
        State {
            pc: 0x200,
            vram,
            memory: Memory { rom },
            stack: Vec::new(),
            registers: Registers([0; 16]),
            vi: 0,
            keypad,
            delay_timer,
            sound_timer,
            last_key_press: None,
        }
    }

    async fn run(&mut self) -> ControlFlow<ExitReason> {
        loop {
            let instr = self.fetch();
            debug!("{:04X}: {instr:04X?}", self.pc);
            let instr = instr.decode();
            //TODO: wait for keypress / Draw sprite?
            match self.execute(instr) {
                ControlFlow::Break(ExitReason::WaitingForKeyPress) => {
                    self.pc -= 2;
                    //TODO Verify behavior
                    loop {
                        let key_pressed = self.keypad.lock().unwrap().first_pressed();

                        if let Some(key) = key_pressed {
                            self.last_key_press = Some(key);
                        } else if self.last_key_press.is_some() {
                            break;
                        }
                        smol::future::yield_now().await;
                    }
                }
                reason => reason?,
            };
            smol::future::yield_now().await;
        }
    }
}

async fn handle_timer(timer: Arc<Mutex<u8>>) -> ! {
    loop {
        {
            let mut timer = timer.lock().unwrap();
            *timer = timer.saturating_sub(1);
        }
        Timer::after(Duration::from_secs_f32(1f32 / 60f32)).await;
    }
}
