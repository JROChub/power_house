#![no_main]
#![no_std]

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

const LANES: usize = 32;
const BYTES: usize = LANES * 4;

#[inline(never)]
fn mix_lane(left: u32, right: u32, round: u32) -> u32 {
    let rotation = (round ^ right) & 31;
    let combined = left.rotate_left(rotation)
        ^ right.rotate_right((left ^ round) & 31)
        ^ round.wrapping_mul(0x9e37_79b9);
    combined.wrapping_mul(0x85eb_ca6b).wrapping_add(0xc2b2_ae35)
}

fn main() {
    let mut private_words = [0_u32; LANES + 2];
    env::read_slice(&mut private_words);

    let rounds = (private_words[0] & 63) + 1;
    let active = ((private_words[1] & 31) + 1) as usize;
    let mut lanes = [0_u32; LANES];
    lanes.copy_from_slice(&private_words[2..]);

    let mut branch_counts = [0_u32; 4];
    for round in 0..rounds {
        let index = (round as usize * 7 + (lanes[round as usize % active] as usize)) % active;
        let peer = (index + 1 + ((lanes[index] >> 27) as usize)) % active;
        let left = lanes[index];
        let right = lanes[peer];

        let mixed = if (left & 1) == 0 {
            branch_counts[0] += 1;
            mix_lane(left ^ right, left | right, round)
        } else if (left as i32) < (right as i32) {
            branch_counts[1] += 1;
            mix_lane(left & right, left.wrapping_add(right), round)
        } else if left != right && left < right {
            branch_counts[2] += 1;
            mix_lane(!left, right ^ round, round)
        } else {
            branch_counts[3] += 1;
            mix_lane(left.wrapping_sub(right), left ^ right, round)
        };

        lanes[index] = mixed;
        lanes[peer] = lanes[peer].wrapping_add(mixed.rotate_right((index as u32) & 31)) ^ round;
    }

    let mut bytes = [0_u8; BYTES];
    for (index, lane) in lanes.iter().enumerate() {
        let offset = index * 4;
        bytes[offset..offset + 4].copy_from_slice(&lane.to_le_bytes());
    }

    let byte_index = (lanes[0] as usize) % BYTES;
    bytes[byte_index] ^= (rounds as u8).wrapping_mul(17);
    let halfword_index = ((lanes[1] as usize) % (BYTES - 1)) & !1;
    let halfword = u16::from_le_bytes([bytes[halfword_index], bytes[halfword_index + 1]])
        .rotate_left((rounds & 15) as u32)
        ^ 0xa55a;
    bytes[halfword_index..halfword_index + 2].copy_from_slice(&halfword.to_le_bytes());

    let mut digest = 0x6a09_e667_u32;
    let mut minimum = u32::MAX;
    let mut signed_negative = 0_u32;
    for chunk in bytes.chunks_exact(4) {
        let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        minimum = minimum.min(word);
        signed_negative += u32::from((word as i32) < 0);
        digest = digest.rotate_left(5) ^ word.wrapping_mul(0x27d4_eb2d);
    }

    let mut byte_checksum = 0_u32;
    for (index, byte) in bytes.iter().enumerate() {
        byte_checksum = byte_checksum.wrapping_add((*byte as u32) * (index as u32 + 1));
    }

    env::commit_slice(&[
        digest,
        minimum,
        signed_negative,
        branch_counts[0],
        branch_counts[1],
        branch_counts[2],
        branch_counts[3],
        byte_checksum,
    ]);
}
