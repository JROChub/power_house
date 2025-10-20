use power_house::Field;

#[allow(dead_code)]
fn lcg_advance_and_sum(field: &Field, p: u64, a: u64, b: u64, s0: u64, n: u64) -> (u64, u64) {
    if n == 0 {
        return (s0, 0);
    }
    let a_mod = a % p;
    let b_mod = b % p;
    let s0_mod = s0 % p;
    let one = 1 % p;
    let pow_a_n = field.pow(a_mod, n);
    let inv = field.inv(field.sub(a_mod, one));
    let a_geom = field.mul(field.sub(pow_a_n, one), inv);
    let s_n = field.add(field.mul(pow_a_n, s0_mod), field.mul(b_mod, a_geom));
    let sum_n = field.add(
        field.mul(s0_mod, a_geom),
        field.mul(
            b_mod,
            field.mul(field.sub(a_geom, n % p), inv),
        ),
    );
    (s_n % p, sum_n % p)
}

fn mix64(mut h: u64, x: u64) -> u64 {
    h ^= x.wrapping_mul(0x9E37_79B1_85EB_CA87);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^ (h >> 29)
}

fn mix64_b(mut h: u64, x: u64) -> u64 {
    h ^= x.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 32;
    h = h.wrapping_mul(0x2545_F491_4F6C_DD1D);
    h ^ (h >> 27)
}

fn inv_mod_u64(a: u64, p: u64) -> u64 {
    fn modpow(mut b: u128, mut e: u128, m: u128) -> u128 {
        let mut r: u128 = 1 % m;
        while e > 0 {
            if e & 1 == 1 {
                r = (r * b) % m;
            }
            b = (b * b) % m;
            e >>= 1;
        }
        r
    }
    modpow(a as u128 % p as u128, (p as u128).wrapping_sub(2), p as u128) as u64
}

fn crt3(r1: u64, p1: u64, r2: u64, p2: u64, r3: u64, p3: u64) -> (u128, u128) {
    let (m1, m2, m3) = (p1 as u128, p2 as u128, p3 as u128);
    let (mut x, _r1u, _r2u, _r3u) = (r1 as u128, r1 as u128, r2 as u128, r3 as u128);
    let inv_m1_p2 = inv_mod_u64((p1 % p2) as u64, p2) as u128;
    let t2 = ((r2 as u128 + m2 - (x % m2)) % m2) * inv_m1_p2 % m2;
    x += t2 * m1;
    let m12_mod_p3 = ((p1 as u128 % m3) * (p2 as u128 % m3)) % m3;
    let inv_m12_p3 = inv_mod_u64(m12_mod_p3 as u64, p3) as u128;
    let t3 = ((r3 as u128 + m3 - (x % m3)) % m3) * inv_m12_p3 % m3;
    x += t3 * (m1 * m2);
    (x % (m1 * m2 * m3), m1 * m2 * m3)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn nck_mod(n: u64, k_in: u64, m: u64) -> u64 {
    if m == 0 {
        return 0;
    }
    if k_in == 0 || k_in == n {
        return 1 % m;
    }
    let k = k_in.min(n - k_in);
    let mut den: Vec<u64> = (1..=k).collect();
    let mut res: u128 = 1 % (m as u128);
    for t in 0..k {
        let mut num = n - t;
        for d in den.iter_mut() {
            if *d > 1 {
                let g = gcd(num, *d);
                if g > 1 {
                    num /= g;
                    *d /= g;
                    if num == 1 {
                        break;
                    }
                }
            }
        }
        res = (res * ((num % m) as u128)) % (m as u128);
    }
    (res % (m as u128)) as u64
}

fn lcg_advance_and_sum_with_residues(
    field: &Field,
    p: u64,
    a: u64,
    b: u64,
    s0: u64,
    n_mod_p: u64,
    n_mod_phi: u64,
) -> (u64, u64) {
    let a_mod = a % p;
    let b_mod = b % p;
    let s0_mod = s0 % p;
    let one = 1 % p;
    let pow_a_n = field.pow(a_mod, n_mod_phi);
    let a_minus_1 = field.sub(a_mod, one);
    let inv_a_minus_1 = field.inv(a_minus_1);
    let a_geom = field.mul(field.sub(pow_a_n, one), inv_a_minus_1);
    let s_n = field.add(field.mul(pow_a_n, s0_mod), field.mul(b_mod, a_geom));
    let a_minus_n = field.sub(a_geom, n_mod_p % p);
    let sum_n = field.add(
        field.mul(s0_mod, a_geom),
        field.mul(b_mod, field.mul(a_minus_n, inv_a_minus_1)),
    );
    (s_n % p, sum_n % p)
}

fn main() {
    let (p1, p2, p3) = (1_000_000_007u64, 1_000_000_009, 1_000_000_021);
    let (f1, f2, f3) = (Field::new(p1), Field::new(p2), Field::new(p3));

    let n: u64 = 10_000_000_000_000_000_000;

    let phi1 = p1 - 1;
    let phi2 = p2 - 1;
    let phi3 = p3 - 1;

    let counts_mod_p1 = [
        1 % p1,
        nck_mod(n, 1, p1),
        nck_mod(n, 2, p1),
        nck_mod(n, 4, p1),
        nck_mod(n, 8, p1),
    ];
    let counts_mod_phi1 = [
        1 % phi1,
        nck_mod(n, 1, phi1),
        nck_mod(n, 2, phi1),
        nck_mod(n, 4, phi1),
        nck_mod(n, 8, phi1),
    ];
    let counts_mod_p2 = [
        1 % p2,
        nck_mod(n, 1, p2),
        nck_mod(n, 2, p2),
        nck_mod(n, 4, p2),
        nck_mod(n, 8, p2),
    ];
    let counts_mod_phi2 = [
        1 % phi2,
        nck_mod(n, 1, phi2),
        nck_mod(n, 2, phi2),
        nck_mod(n, 4, phi2),
        nck_mod(n, 8, phi2),
    ];
    let counts_mod_p3 = [
        1 % p3,
        nck_mod(n, 1, p3),
        nck_mod(n, 2, p3),
        nck_mod(n, 4, p3),
        nck_mod(n, 8, p3),
    ];
    let counts_mod_phi3 = [
        1 % phi3,
        nck_mod(n, 1, phi3),
        nck_mod(n, 2, phi3),
        nck_mod(n, 4, phi3),
        nck_mod(n, 8, phi3),
    ];

    let w1_0 = f1.pow(2, n);
    let w1_1 = f1.pow(2, n - 1);
    let w1_2 = f1.pow(2, n - 2);
    let w1_4 = f1.pow(2, n - 4);
    let w1_8 = f1.pow(2, n - 8);

    let w2_0 = f2.pow(2, n);
    let w2_1 = f2.pow(2, n - 1);
    let w2_2 = f2.pow(2, n - 2);
    let w2_4 = f2.pow(2, n - 4);
    let w2_8 = f2.pow(2, n - 8);

    let w3_0 = f3.pow(2, n);
    let w3_1 = f3.pow(2, n - 1);
    let w3_2 = f3.pow(2, n - 2);
    let w3_4 = f3.pow(2, n - 4);
    let w3_8 = f3.pow(2, n - 8);

    let mut a1 = 6_364_136_223_846_793_005u64 % p1;
    let mut b1 = 1_442_695_040_888_963_407 % p1;
    let mut a2 = 6_364_136_223_846_793_005u64 % p2;
    let mut b2 = 1_442_695_040_888_963_407 % p2;
    let mut a3 = 6_364_136_223_846_793_005u64 % p3;
    let mut b3 = 1_442_695_040_888_963_407 % p3;
    let base_seed: u64 = 0x504F_5745_525F_484F ^ 0x0055_5345;

    let mut s1 = base_seed % p1;
    let mut s2 = base_seed % p2;
    let mut s3 = base_seed % p3;

    let rounds: usize = 12;
    println!("Power-House CRT Chain (n={n}, rounds={rounds})");
    println!("================================================");
    for r in 0..rounds {
        let (s1a, c1_0) =
            lcg_advance_and_sum_with_residues(&f1, p1, a1, b1, s1, counts_mod_p1[0], counts_mod_phi1[0]);
        let (s1b, c1_1) =
            lcg_advance_and_sum_with_residues(&f1, p1, a1, b1, s1a, counts_mod_p1[1], counts_mod_phi1[1]);
        let (s1c, c1_2) =
            lcg_advance_and_sum_with_residues(&f1, p1, a1, b1, s1b, counts_mod_p1[2], counts_mod_phi1[2]);
        let (s1d, c1_4) =
            lcg_advance_and_sum_with_residues(&f1, p1, a1, b1, s1c, counts_mod_p1[3], counts_mod_phi1[3]);
        let (s1e, c1_8) =
            lcg_advance_and_sum_with_residues(&f1, p1, a1, b1, s1d, counts_mod_p1[4], counts_mod_phi1[4]);
        s1 = s1e;
        let t1 = {
            let t0 = f1.mul(c1_0 % p1, w1_0);
            let t_a = f1.mul(c1_1, w1_1);
            let t_b = f1.mul(c1_2, w1_2);
            let t_c = f1.mul(c1_4, w1_4);
            let t_d = f1.mul(c1_8, w1_8);
            f1.add(f1.add(t0, t_a), f1.add(f1.add(t_b, t_c), t_d))
        };

        let (s2a, c2_0) =
            lcg_advance_and_sum_with_residues(&f2, p2, a2, b2, s2, counts_mod_p2[0], counts_mod_phi2[0]);
        let (s2b, c2_1) =
            lcg_advance_and_sum_with_residues(&f2, p2, a2, b2, s2a, counts_mod_p2[1], counts_mod_phi2[1]);
        let (s2c, c2_2) =
            lcg_advance_and_sum_with_residues(&f2, p2, a2, b2, s2b, counts_mod_p2[2], counts_mod_phi2[2]);
        let (s2d, c2_4) =
            lcg_advance_and_sum_with_residues(&f2, p2, a2, b2, s2c, counts_mod_p2[3], counts_mod_phi2[3]);
        let (s2e, c2_8) =
            lcg_advance_and_sum_with_residues(&f2, p2, a2, b2, s2d, counts_mod_p2[4], counts_mod_phi2[4]);
        s2 = s2e;
        let t2 = {
            let t0 = f2.mul(c2_0 % p2, w2_0);
            let t_a = f2.mul(c2_1, w2_1);
            let t_b = f2.mul(c2_2, w2_2);
            let t_c = f2.mul(c2_4, w2_4);
            let t_d = f2.mul(c2_8, w2_8);
            f2.add(f2.add(t0, t_a), f2.add(f2.add(t_b, t_c), t_d))
        };

        let (s3a, c3_0) =
            lcg_advance_and_sum_with_residues(&f3, p3, a3, b3, s3, counts_mod_p3[0], counts_mod_phi3[0]);
        let (s3b, c3_1) =
            lcg_advance_and_sum_with_residues(&f3, p3, a3, b3, s3a, counts_mod_p3[1], counts_mod_phi3[1]);
        let (s3c, c3_2) =
            lcg_advance_and_sum_with_residues(&f3, p3, a3, b3, s3b, counts_mod_p3[2], counts_mod_phi3[2]);
        let (s3d, c3_4) =
            lcg_advance_and_sum_with_residues(&f3, p3, a3, b3, s3c, counts_mod_p3[3], counts_mod_phi3[3]);
        let (s3e, c3_8) =
            lcg_advance_and_sum_with_residues(&f3, p3, a3, b3, s3d, counts_mod_p3[4], counts_mod_phi3[4]);
        s3 = s3e;
        let t3 = {
            let t0 = f3.mul(c3_0 % p3, w3_0);
            let t_a = f3.mul(c3_1, w3_1);
            let t_b = f3.mul(c3_2, w3_2);
            let t_c = f3.mul(c3_4, w3_4);
            let t_d = f3.mul(c3_8, w3_8);
            f3.add(f3.add(t0, t_a), f3.add(f3.add(t_b, t_c), t_d))
        };

        let (crt_val, _) = crt3(t1, p1, t2, p2, t3, p3);
        let mut h1 = 0xDEAD_BEEF_CAFE_BABEu64;
        let mut h2 = 0xA11C_EB0B_AC1A_AB13u64;
        for &x in &[
            p1,
            p2,
            p3,
            n,
            c1_0 % p1,
            c1_1,
            c1_2,
            c1_4,
            c1_8,
            t1,
            c2_0 % p2,
            c2_1,
            c2_2,
            c2_4,
            c2_8,
            t2,
            c3_0 % p3,
            c3_1,
            c3_2,
            c3_4,
            c3_8,
            t3,
        ] {
            h1 = mix64(h1, x);
            h2 = mix64_b(h2, x);
        }
        h1 = mix64(h1, (crt_val & 0xFFFF_FFFF_FFFF_FFFFu128) as u64);
        h2 = mix64_b(h2, (crt_val >> 64) as u64);

        println!(
            "round {r:02}: totals=({}, {}, {}), digest={:016X}-{:016X}",
            t1, t2, t3, h1, h2
        );

        let (mut a1_new, mut b1_new) = (a1, b1);
        let (mut a2_new, mut b2_new) = (a2, b2);
        let (mut a3_new, mut b3_new) = (a3, b3);
        a1_new = ((a1_new as u128 + ((h1 as u128) | 1)) % p1 as u128) as u64;
        if a1_new == 1 {
            a1_new = 2;
        }
        b1_new = ((b1_new as u128 + h2 as u128) % p1 as u128) as u64;
        a2_new = ((a2_new as u128 + ((h2 as u128) | 1)) % p2 as u128) as u64;
        if a2_new == 1 {
            a2_new = 2;
        }
        b2_new = ((b2_new as u128 + h1 as u128) % p2 as u128) as u64;
        a3_new = ((a3_new as u128 + (((h1 ^ h2) as u128) | 1)) % p3 as u128) as u64;
        if a3_new == 1 {
            a3_new = 2;
        }
        b3_new = ((b3_new as u128 + (h1.wrapping_add(h2)) as u128) % p3 as u128) as u64;
        a1 = a1_new;
        b1 = b1_new;
        a2 = a2_new;
        b2 = b2_new;
        a3 = a3_new;
        b3 = b3_new;

        s1 = ((s1 as u128 ^ h1 as u128 ^ h2 as u128) % p1 as u128) as u64;
        if s1 == 0 {
            s1 = 1;
        }
        s2 = ((s2 as u128 ^ ((h1 as u128).wrapping_mul(3)) ^ h2 as u128) % p2 as u128) as u64;
        if s2 == 0 {
            s2 = 1;
        }
        s3 = ((s3 as u128 ^ h1 as u128 ^ ((h2 as u128).wrapping_mul(5))) % p3 as u128) as u64;
        if s3 == 0 {
            s3 = 1;
        }
    }
}
