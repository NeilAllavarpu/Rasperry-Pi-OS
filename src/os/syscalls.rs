pub fn alloc_page(_page_size: u64) -> Result<u64, ()> {
    Ok(0x80_0000)
}
