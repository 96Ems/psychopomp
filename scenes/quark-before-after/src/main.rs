fn main() -> Result<(), Box<dyn std::error::Error>> {
    psychopomp_quark_before_after::build_plan()?.write_or_print(std::env::args().nth(1))?;
    Ok(())
}
