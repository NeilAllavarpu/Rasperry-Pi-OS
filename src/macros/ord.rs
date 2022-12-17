/// Automatically implements `ParitalEq`, `Eq`, and `PartialOrd` from an `Ord` implementation
#[macro_export]
#[allow(clippy::module_name_repetitions)]
macro_rules! derive_ord {
    ($struct:ty) => {
        impl PartialEq for $struct {
            fn eq(&self, other: &Self) -> bool {
                self.cmp(other).is_eq()
            }
        }

        impl Eq for $struct {}

        impl PartialOrd for $struct {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}
