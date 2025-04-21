/// Writes a byte to the console.
pub fn putchar(c: u8) {
    // #[allow(deprecated)]
    // for b in b"\x1b[31m" {
    //     sbi_rt::legacy::console_putchar(*b as usize);
    // }
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
    // #[allow(deprecated)]
    // for b in b"\x1b[31m" {
    //     sbi_rt::legacy::console_putchar(*b as usize);
    // }
}

/// Reads a byte from the console, or returns [`None`] if no input is available.
pub fn getchar() -> Option<u8> {
    #[allow(deprecated)]
    match sbi_rt::legacy::console_getchar() as isize {
        -1 => None,
        c => Some(c as u8),
    }
}