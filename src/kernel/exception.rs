#[derive(PartialEq, Debug)]

/// Represents the privilege level of some execution context
pub enum PrivilegeLevel {
    /// Lowest privilege mode
    User,
    /// OS privilege mode
    Kernel,
    /// Privilege mode above OS; may or may not exist
    Hypervisor,
    /// Unknown privilege level
    Unknown,
}
