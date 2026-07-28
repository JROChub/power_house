#![no_main]
#![no_std]

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

fn main() {
    let mut private_values = [0_u32; 2];
    env::read_slice(&mut private_values);
    let sum = private_values[0].wrapping_add(private_values[1]);
    env::commit(&sum);
}
