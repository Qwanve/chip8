# Chip-8 Emulator
An interpreted emulator for the Chip-8 system written in Rust. 

This has been a fun project to work on for the past month or so, and it's just reached what I would consider the minimum viable stage. I'm fairly certain I've implemented all the instructions and input/output, but it's still in the early stages. It definitely has bugs and lots of naive implementation details, but it runs Tetris and Mastermind, and [Timendus' test suite](https://github.com/Timendus/chip8-test-suite).

## Future plans
1. Super-chip and XO-chip features
2. Palette choice
3. Wasm and/or TUI implementation
4. Better debugging tools (i.e. breakpoints, memory view, etc)
5. Profiling and optimization
6. Automated testing

## Special thanks

Timendus' test suite was a god send. Having a set of good roms to test against and having a good implementation to check behaviors for was invaluable.

Matthew Mikolay's CHIP‐8 Technical Reference. Great store of knowledge that answered nearly every question I had.

