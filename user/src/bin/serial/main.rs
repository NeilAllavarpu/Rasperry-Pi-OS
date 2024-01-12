#![no_std]
#![no_main]

use user::println;

#[no_mangle]
extern "C" fn main() {
    println!("Hello, world!");
}
