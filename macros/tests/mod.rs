#[cfg(test)]
mod tests {
    const FIRST_VALUE: u16 = 0;
    const SECOND_VALUE: u16 = 13;
    const THIRD_VALUE: u16 = 12345;

    #[derive(macros::AsBits, PartialEq, Debug)]
    #[repr(u16)]
    enum Enum {
        First = FIRST_VALUE,
        Second = SECOND_VALUE,
        Third = THIRD_VALUE,
    }

    #[test]
    fn into_bits() {
        assert_eq!(Enum::First.into_bits(), FIRST_VALUE);
        assert_eq!(Enum::Second.into_bits(), SECOND_VALUE);
        assert_eq!(Enum::Third.into_bits(), THIRD_VALUE);
    }

    #[test]
    fn from_bits_valid() {
        assert_eq!(Enum::from_bits(FIRST_VALUE), Enum::First);
        assert_eq!(Enum::from_bits(SECOND_VALUE), Enum::Second);
        assert_eq!(Enum::from_bits(THIRD_VALUE), Enum::Third);
    }

    #[test]
    #[should_panic]
    fn from_bits_invalid() {
        for i in 0.. {
            if ![FIRST_VALUE, SECOND_VALUE, THIRD_VALUE]
                .iter()
                .any(|&value| i == value)
            {
                Enum::from_bits(i);
            }
        }
    }
}
