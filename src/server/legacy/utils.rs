/// Converts bool to [u8] representation
/// * [false] to `0`
/// * [true] to `1`
pub fn bool_to_u8(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}