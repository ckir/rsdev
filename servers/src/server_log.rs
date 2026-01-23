use lib_common::beep_with_hz_and_millis;
use lib_common::logrecord::Logrecord;

fn main() {
    let middle_e_hz = 329;
    let a_bit_more_than_a_second_and_a_half_ms = 1600;

    beep_with_hz_and_millis(middle_e_hz, a_bit_more_than_a_second_and_a_half_ms).unwrap();

    println!("Hello, world!");
    let logrecord: Logrecord = Logrecord::default();
    println!("{:?}", logrecord);
}
